use serde::{Deserialize, Serialize};

/// Mouse button identifiers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// A rectangular screen region for targeted capture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScreenRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Actions an agent can execute on the desktop.
/// Every variant maps to a capability requirement and a token cost.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComputerAction {
    Screenshot { region: Option<ScreenRegion> },
    MouseMove { x: u32, y: u32 },
    MouseClick { x: u32, y: u32, button: MouseButton },
    MouseDoubleClick { x: u32, y: u32 },
    KeyboardType { text: String },
    KeyboardShortcut { keys: Vec<String> },
    TerminalCommand { command: String, working_dir: String },
    ReadClipboard,
    WriteClipboard { content: String },
    OpenApplication { app_name: String },
    WaitForElement { selector: String, timeout_ms: u64 },
}

impl ComputerAction {
    /// Short human-readable label for audit logging.
    pub fn label(&self) -> String {
        match self {
            Self::Screenshot { region } => {
                if let Some(r) = region {
                    format!("screenshot({}x{} @ {},{})", r.width, r.height, r.x, r.y)
                } else {
                    "screenshot(full)".into()
                }
            }
            Self::MouseMove { x, y } => format!("mouse_move({x},{y})"),
            Self::MouseClick { x, y, button } => format!("mouse_click({button:?} @ {x},{y})"),
            Self::MouseDoubleClick { x, y } => format!("double_click({x},{y})"),
            Self::KeyboardType { text } => {
                let preview: String = text.chars().take(20).collect();
                let suffix = if text.chars().count() > 20 { "..." } else { "" };
                format!("type(\"{preview}{suffix}\")")
            }
            Self::KeyboardShortcut { keys } => format!("shortcut({})", keys.join("+")),
            Self::TerminalCommand { command, .. } => {
                let preview: String = command.chars().take(30).collect();
                let suffix = if command.chars().count() > 30 {
                    "..."
                } else {
                    ""
                };
                format!("terminal(\"{preview}{suffix}\")")
            }
            Self::ReadClipboard => "read_clipboard".into(),
            Self::WriteClipboard { .. } => "write_clipboard".into(),
            Self::OpenApplication { app_name } => format!("open_app({app_name})"),
            Self::WaitForElement { selector, .. } => format!("wait_for({selector})"),
        }
    }
}
