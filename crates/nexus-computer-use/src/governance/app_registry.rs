use std::collections::HashMap;
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::warn;

use crate::error::ComputerUseError;

/// Information about a running application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    /// Human-readable name (e.g., "Firefox", "Terminal")
    pub name: String,
    /// X11 WM_CLASS for identification
    pub wm_class: String,
    /// Process ID
    pub pid: u32,
    /// X11 window ID
    pub window_id: u64,
    /// Current window title
    pub title: String,
    /// Detected category
    pub category: AppCategory,
    /// Whether this window is currently focused
    pub is_focused: bool,
}

/// Category of application for governance defaults
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AppCategory {
    Terminal,
    Editor,
    Browser,
    FileManager,
    Communication,
    System,
    NexusOS,
    Unknown,
}

impl std::fmt::Display for AppCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Terminal => write!(f, "Terminal"),
            Self::Editor => write!(f, "Editor"),
            Self::Browser => write!(f, "Browser"),
            Self::FileManager => write!(f, "FileManager"),
            Self::Communication => write!(f, "Communication"),
            Self::System => write!(f, "System"),
            Self::NexusOS => write!(f, "NexusOS"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Registry of known applications and their categories
pub struct AppRegistry {
    known_apps: HashMap<String, AppCategory>,
}

impl Default for AppRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AppRegistry {
    /// Create a new registry with default known apps
    pub fn new() -> Self {
        let mut known_apps = HashMap::new();
        // Terminals
        for name in &[
            "gnome-terminal",
            "kitty",
            "alacritty",
            "xterm",
            "konsole",
            "terminator",
            "wezterm",
            "foot",
        ] {
            known_apps.insert(name.to_string(), AppCategory::Terminal);
        }
        // Editors
        for name in &[
            "code",
            "vscodium",
            "cursor",
            "vim",
            "neovim",
            "emacs",
            "gedit",
            "sublime_text",
            "jetbrains",
        ] {
            known_apps.insert(name.to_string(), AppCategory::Editor);
        }
        // Browsers
        for name in &["firefox", "chrome", "chromium", "brave", "opera", "vivaldi"] {
            known_apps.insert(name.to_string(), AppCategory::Browser);
        }
        // File managers
        for name in &["nautilus", "thunar", "nemo", "dolphin", "pcmanfm"] {
            known_apps.insert(name.to_string(), AppCategory::FileManager);
        }
        // Communication
        for name in &[
            "slack",
            "discord",
            "telegram",
            "teams",
            "signal",
            "thunderbird",
        ] {
            known_apps.insert(name.to_string(), AppCategory::Communication);
        }
        // Nexus OS
        for name in &["nexus", "tauri"] {
            known_apps.insert(name.to_string(), AppCategory::NexusOS);
        }
        // System
        for name in &[
            "settings",
            "monitor",
            "task",
            "gnome-control-center",
            "systemsettings",
        ] {
            known_apps.insert(name.to_string(), AppCategory::System);
        }

        Self { known_apps }
    }

    /// Register a custom app category mapping
    pub fn register_app(&mut self, wm_class_pattern: &str, category: AppCategory) {
        self.known_apps
            .insert(wm_class_pattern.to_lowercase(), category);
    }

    /// Categorize an app by its WM_CLASS string
    pub fn categorize(&self, wm_class: &str) -> AppCategory {
        let lower = wm_class.to_lowercase();

        // Check known apps first (exact substring match)
        for (pattern, category) in &self.known_apps {
            if lower.contains(pattern) {
                return category.clone();
            }
        }

        // Fallback heuristic matching
        categorize_heuristic(&lower)
    }

    /// Get information about the currently focused window
    pub async fn get_focused_app(&self) -> Result<AppInfo, ComputerUseError> {
        let window_id = run_xdotool(&["getactivewindow"]).await?;
        let window_id_num = window_id
            .trim()
            .parse::<u64>()
            .map_err(|e| ComputerUseError::InputError(format!("Invalid window id: {e}")))?;

        let title = run_xdotool(&["getactivewindow", "getwindowname"]).await?;
        let pid_str = run_xdotool(&["getactivewindow", "getwindowpid"]).await?;
        let pid = pid_str
            .trim()
            .parse::<u32>()
            .map_err(|e| ComputerUseError::InputError(format!("Invalid PID: {e}")))?;

        let wm_class = get_wm_class(window_id_num).await?;
        let category = self.categorize(&wm_class);

        // Derive a friendly name from the WM_CLASS
        let name = derive_app_name(&wm_class, &title);

        Ok(AppInfo {
            name,
            wm_class,
            pid,
            window_id: window_id_num,
            title: title.trim().to_string(),
            category,
            is_focused: true,
        })
    }

    /// List all visible windows with their app info
    pub async fn list_visible_apps(&self) -> Result<Vec<AppInfo>, ComputerUseError> {
        let output = run_xdotool(&["search", "--onlyvisible", "--name", ""]).await?;
        let active_id = run_xdotool(&["getactivewindow"])
            .await
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0);

        let mut apps = Vec::new();
        for line in output.lines() {
            let window_id = match line.trim().parse::<u64>() {
                Ok(id) => id,
                Err(_) => continue,
            };

            let title = run_xdotool(&["getwindowname", &window_id.to_string()])
                .await
                .unwrap_or_default();
            let pid_str = run_xdotool(&["getwindowpid", &window_id.to_string()])
                .await
                .unwrap_or_default();
            let pid = pid_str.trim().parse::<u32>().unwrap_or(0);
            let wm_class = get_wm_class(window_id).await.unwrap_or_default();
            let category = self.categorize(&wm_class);
            let name = derive_app_name(&wm_class, &title);

            apps.push(AppInfo {
                name,
                wm_class,
                pid,
                window_id,
                title: title.trim().to_string(),
                category,
                is_focused: window_id == active_id,
            });
        }

        Ok(apps)
    }
}

/// Heuristic categorization when no known app pattern matches
fn categorize_heuristic(lower: &str) -> AppCategory {
    if lower.contains("terminal")
        || lower.contains("kitty")
        || lower.contains("alacritty")
        || lower.contains("gnome-terminal")
        || lower.contains("xterm")
        || lower.contains("konsole")
    {
        AppCategory::Terminal
    } else if lower.contains("code")
        || lower.contains("vscodium")
        || lower.contains("cursor")
        || lower.contains("vim")
        || lower.contains("neovim")
        || lower.contains("emacs")
    {
        AppCategory::Editor
    } else if lower.contains("firefox")
        || lower.contains("chrome")
        || lower.contains("chromium")
        || lower.contains("brave")
    {
        AppCategory::Browser
    } else if lower.contains("nautilus")
        || lower.contains("thunar")
        || lower.contains("nemo")
        || lower.contains("dolphin")
    {
        AppCategory::FileManager
    } else if lower.contains("slack")
        || lower.contains("discord")
        || lower.contains("telegram")
        || lower.contains("teams")
    {
        AppCategory::Communication
    } else if lower.contains("nexus") || lower.contains("tauri") {
        AppCategory::NexusOS
    } else if lower.contains("settings") || lower.contains("monitor") || lower.contains("task") {
        AppCategory::System
    } else {
        AppCategory::Unknown
    }
}

/// Derive a friendly app name from WM_CLASS and title
fn derive_app_name(wm_class: &str, title: &str) -> String {
    // WM_CLASS often has format "instance, class" — use the class part
    if let Some((_instance, class)) = wm_class.split_once(", ") {
        return class.trim().trim_matches('"').to_string();
    }
    // Fall back to first word of the WM_CLASS
    let cleaned = wm_class.trim().trim_matches('"');
    if !cleaned.is_empty() {
        return cleaned.to_string();
    }
    // Last resort: first part of the window title
    title
        .split(['-', '—', '|'])
        .next_back()
        .unwrap_or("Unknown")
        .trim()
        .to_string()
}

/// Run an xdotool command with timeout
async fn run_xdotool(args: &[&str]) -> Result<String, ComputerUseError> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        Command::new("xdotool")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| ComputerUseError::Timeout { seconds: 5 })?
    .map_err(|e| ComputerUseError::InputError(format!("xdotool failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("xdotool {:?} failed: {}", args, stderr);
        return Err(ComputerUseError::InputError(format!(
            "xdotool {:?} failed: {}",
            args, stderr
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Get WM_CLASS for a window using xprop
async fn get_wm_class(window_id: u64) -> Result<String, ComputerUseError> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        Command::new("xprop")
            .args(["-id", &window_id.to_string(), "WM_CLASS"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| ComputerUseError::Timeout { seconds: 5 })?
    .map_err(|e| ComputerUseError::InputError(format!("xprop failed: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    // xprop output: WM_CLASS(STRING) = "instance", "class"
    if let Some(eq_pos) = stdout.find('=') {
        Ok(stdout[eq_pos + 1..].trim().to_string())
    } else {
        Ok(String::new())
    }
}

/// Parse a WM_CLASS string into (instance, class) tuple
pub fn parse_wm_class(raw: &str) -> (String, String) {
    let parts: Vec<&str> = raw.split(',').collect();
    let instance = parts
        .first()
        .unwrap_or(&"")
        .trim()
        .trim_matches('"')
        .to_string();
    let class = parts
        .get(1)
        .unwrap_or(&"")
        .trim()
        .trim_matches('"')
        .to_string();
    (instance, class)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_terminal_gnome() {
        let registry = AppRegistry::new();
        assert_eq!(
            registry.categorize("\"gnome-terminal-server\", \"Gnome-terminal\""),
            AppCategory::Terminal
        );
    }

    #[test]
    fn test_categorize_terminal_kitty() {
        let registry = AppRegistry::new();
        assert_eq!(
            registry.categorize("\"kitty\", \"kitty\""),
            AppCategory::Terminal
        );
    }

    #[test]
    fn test_categorize_editor_vscode() {
        let registry = AppRegistry::new();
        assert_eq!(
            registry.categorize("\"code\", \"Code\""),
            AppCategory::Editor
        );
    }

    #[test]
    fn test_categorize_browser_firefox() {
        let registry = AppRegistry::new();
        assert_eq!(
            registry.categorize("\"Navigator\", \"firefox\""),
            AppCategory::Browser
        );
    }

    #[test]
    fn test_categorize_browser_chrome() {
        let registry = AppRegistry::new();
        assert_eq!(
            registry.categorize("\"google-chrome\", \"Google-chrome\""),
            AppCategory::Browser
        );
    }

    #[test]
    fn test_categorize_nexus_os() {
        let registry = AppRegistry::new();
        assert_eq!(
            registry.categorize("\"nexus-os\", \"Nexus OS\""),
            AppCategory::NexusOS
        );
    }

    #[test]
    fn test_categorize_unknown_app() {
        let registry = AppRegistry::new();
        assert_eq!(
            registry.categorize("\"randomapp\", \"RandomApp\""),
            AppCategory::Unknown
        );
    }

    #[test]
    fn test_app_info_creation() {
        let info = AppInfo {
            name: "Firefox".to_string(),
            wm_class: "\"Navigator\", \"firefox\"".to_string(),
            pid: 1234,
            window_id: 0x0400_0001,
            title: "Nexus OS - Mozilla Firefox".to_string(),
            category: AppCategory::Browser,
            is_focused: true,
        };
        assert_eq!(info.name, "Firefox");
        assert_eq!(info.category, AppCategory::Browser);
        assert!(info.is_focused);
        assert_eq!(info.pid, 1234);
    }

    #[test]
    fn test_parse_wm_class() {
        let (instance, class) = parse_wm_class("\"kitty\", \"kitty\"");
        assert_eq!(instance, "kitty");
        assert_eq!(class, "kitty");

        let (instance, class) = parse_wm_class("\"gnome-terminal-server\", \"Gnome-terminal\"");
        assert_eq!(instance, "gnome-terminal-server");
        assert_eq!(class, "Gnome-terminal");
    }

    #[test]
    fn test_categorize_communication_slack() {
        let registry = AppRegistry::new();
        assert_eq!(
            registry.categorize("\"slack\", \"Slack\""),
            AppCategory::Communication
        );
    }

    #[test]
    fn test_categorize_communication_discord() {
        let registry = AppRegistry::new();
        assert_eq!(
            registry.categorize("\"discord\", \"Discord\""),
            AppCategory::Communication
        );
    }

    #[test]
    fn test_categorize_file_manager_nautilus() {
        let registry = AppRegistry::new();
        assert_eq!(
            registry.categorize("\"org.gnome.Nautilus\", \"nautilus\""),
            AppCategory::FileManager
        );
    }

    #[test]
    fn test_categorize_system_settings() {
        let registry = AppRegistry::new();
        assert_eq!(
            registry.categorize("\"gnome-control-center\", \"Gnome-control-center\""),
            AppCategory::System
        );
    }

    #[test]
    fn test_register_custom_app() {
        let mut registry = AppRegistry::new();
        registry.register_app("myapp", AppCategory::NexusOS);
        assert_eq!(
            registry.categorize("\"myapp\", \"MyApp\""),
            AppCategory::NexusOS
        );
    }

    #[test]
    fn test_category_display() {
        assert_eq!(AppCategory::Terminal.to_string(), "Terminal");
        assert_eq!(AppCategory::Browser.to_string(), "Browser");
        assert_eq!(AppCategory::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn test_derive_app_name_from_wm_class() {
        let name = derive_app_name("\"kitty\", \"kitty\"", "some title");
        assert_eq!(name, "kitty");
    }

    #[test]
    fn test_derive_app_name_fallback_to_title() {
        let name = derive_app_name("", "Firefox - Home");
        assert_eq!(name, "Home");
    }

    #[tokio::test]
    #[ignore] // Requires X11 display
    async fn test_real_list_windows() {
        let registry = AppRegistry::new();
        let apps = registry.list_visible_apps().await;
        assert!(apps.is_ok(), "Failed to list windows: {:?}", apps.err());
        let apps = apps.expect("already checked");
        println!("Found {} visible windows:", apps.len());
        for app in &apps {
            println!(
                "  {} ({}): {} [{}] focused={}",
                app.name, app.wm_class, app.title, app.category, app.is_focused
            );
        }
    }

    #[tokio::test]
    #[ignore] // Requires X11 display
    async fn test_real_focused_app() {
        let registry = AppRegistry::new();
        let app = registry.get_focused_app().await;
        assert!(app.is_ok(), "Failed to get focused app: {:?}", app.err());
        let app = app.expect("already checked");
        println!(
            "Focused: {} ({}) - {} [{}]",
            app.name, app.wm_class, app.title, app.category
        );
    }

    #[tokio::test]
    #[ignore] // Requires X11 display
    async fn test_real_wm_class_detection() {
        let registry = AppRegistry::new();
        let app = registry.get_focused_app().await;
        assert!(app.is_ok());
        let app = app.expect("already checked");
        assert!(!app.wm_class.is_empty(), "WM_CLASS should not be empty");
        // Category should be detected
        println!("WM_CLASS: {} -> category: {}", app.wm_class, app.category);
    }
}
