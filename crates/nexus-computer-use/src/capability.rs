/// Computer use capabilities that can be granted via the ACL
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ComputerCapability {
    /// Take screenshots of the display
    ScreenCapture,
    /// Phase 2: click, move, scroll
    MouseControl,
    /// Phase 2: type, key press
    KeyboardControl,
    /// Phase 4: open applications
    AppLaunch,
    /// Read from system clipboard
    ClipboardRead,
    /// Write to system clipboard
    ClipboardWrite,
}

impl std::fmt::Display for ComputerCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComputerCapability::ScreenCapture => write!(f, "screen_capture"),
            ComputerCapability::MouseControl => write!(f, "mouse_control"),
            ComputerCapability::KeyboardControl => write!(f, "keyboard_control"),
            ComputerCapability::AppLaunch => write!(f, "app_launch"),
            ComputerCapability::ClipboardRead => write!(f, "clipboard_read"),
            ComputerCapability::ClipboardWrite => write!(f, "clipboard_write"),
        }
    }
}

/// Fuel costs for each computer use operation
pub fn fuel_cost(op: &ComputerCapability) -> u64 {
    match op {
        ComputerCapability::ScreenCapture => 1,
        ComputerCapability::MouseControl => 1,
        ComputerCapability::KeyboardControl => 1,
        ComputerCapability::AppLaunch => 2,
        ComputerCapability::ClipboardRead => 1,
        ComputerCapability::ClipboardWrite => 1,
    }
}

/// System requirements check result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemRequirements {
    /// Detected display server: "wayland", "x11", or None
    pub display_server: Option<String>,
    /// Detected capture tool: "grim", "scrot", "import", or None
    pub capture_tool: Option<String>,
    /// Detected input tool (Phase 2): "ydotool", "xdotool", or None
    pub input_tool: Option<String>,
    /// All requirements for screen capture are met
    pub all_capture_ready: bool,
    /// All requirements for input control are met (Phase 2)
    pub all_input_ready: bool,
}

/// Check which system tools are available for computer use
pub fn check_system_requirements() -> SystemRequirements {
    use crate::capture::backend::{detect_backend, detect_display_server, DisplayServer};

    let display = detect_display_server();
    let display_server = match display {
        DisplayServer::Wayland => Some("wayland".to_string()),
        DisplayServer::X11 => Some("x11".to_string()),
        DisplayServer::Unknown => None,
    };

    let backend = detect_backend().ok();
    let capture_tool = backend.map(|b| b.kind.to_string());

    // Phase 2: input tool detection
    let input_tool = detect_input_tool(&display);

    let all_capture_ready = display_server.is_some() && capture_tool.is_some();
    let all_input_ready = display_server.is_some() && input_tool.is_some();

    SystemRequirements {
        display_server,
        capture_tool,
        input_tool,
        all_capture_ready,
        all_input_ready,
    }
}

fn detect_input_tool(display: &crate::capture::backend::DisplayServer) -> Option<String> {
    use crate::capture::backend::DisplayServer;
    use std::process::{Command, Stdio};

    let tool = match display {
        DisplayServer::Wayland => "ydotool",
        DisplayServer::X11 => "xdotool",
        DisplayServer::Unknown => return None,
    };

    let found = Command::new("which")
        .arg(tool)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if found {
        Some(tool.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuel_cost_screen_capture() {
        assert_eq!(fuel_cost(&ComputerCapability::ScreenCapture), 1);
    }

    #[test]
    fn test_fuel_cost_mouse() {
        assert_eq!(fuel_cost(&ComputerCapability::MouseControl), 1);
    }

    #[test]
    fn test_fuel_cost_keyboard() {
        assert_eq!(fuel_cost(&ComputerCapability::KeyboardControl), 1);
    }

    #[test]
    fn test_fuel_cost_app_launch() {
        assert_eq!(fuel_cost(&ComputerCapability::AppLaunch), 2);
    }

    #[test]
    fn test_fuel_cost_clipboard_read() {
        assert_eq!(fuel_cost(&ComputerCapability::ClipboardRead), 1);
    }

    #[test]
    fn test_fuel_cost_clipboard_write() {
        assert_eq!(fuel_cost(&ComputerCapability::ClipboardWrite), 1);
    }

    #[test]
    fn test_capability_enum_variants() {
        // Ensure all 6 variants exist and are distinct
        let variants = [
            ComputerCapability::ScreenCapture,
            ComputerCapability::MouseControl,
            ComputerCapability::KeyboardControl,
            ComputerCapability::AppLaunch,
            ComputerCapability::ClipboardRead,
            ComputerCapability::ClipboardWrite,
        ];
        // All unique
        for i in 0..variants.len() {
            for j in (i + 1)..variants.len() {
                assert_ne!(variants[i], variants[j]);
            }
        }
        assert_eq!(variants.len(), 6);
    }

    #[test]
    fn test_capability_display() {
        assert_eq!(
            ComputerCapability::ScreenCapture.to_string(),
            "screen_capture"
        );
        assert_eq!(
            ComputerCapability::MouseControl.to_string(),
            "mouse_control"
        );
        assert_eq!(ComputerCapability::AppLaunch.to_string(), "app_launch");
    }

    #[test]
    fn test_system_requirements_struct() {
        let req = SystemRequirements {
            display_server: Some("x11".into()),
            capture_tool: Some("scrot".into()),
            input_tool: Some("xdotool".into()),
            all_capture_ready: true,
            all_input_ready: true,
        };
        assert_eq!(req.display_server.as_deref(), Some("x11"));
        assert!(req.all_capture_ready);
        assert!(req.all_input_ready);
    }

    #[test]
    fn test_check_requirements_reports_tools() {
        let req = check_system_requirements();
        // The struct should be populated (values depend on the system)
        // But the function should not panic
        if req.display_server.is_some() {
            assert!(
                req.display_server.as_deref() == Some("wayland")
                    || req.display_server.as_deref() == Some("x11")
            );
        }
    }

    #[test]
    fn test_capability_serialization() {
        let cap = ComputerCapability::ScreenCapture;
        let json = serde_json::to_string(&cap).unwrap();
        let back: ComputerCapability = serde_json::from_str(&json).unwrap();
        assert_eq!(cap, back);
    }
}
