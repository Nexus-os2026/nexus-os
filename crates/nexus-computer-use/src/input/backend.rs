use std::process::Stdio;

use tracing::info;

use crate::error::ComputerUseError;

/// Supported input backends
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputBackendKind {
    /// X11 — xdotool
    Xdotool,
    /// Wayland — ydotool (future)
    Ydotool,
}

impl std::fmt::Display for InputBackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputBackendKind::Xdotool => write!(f, "xdotool"),
            InputBackendKind::Ydotool => write!(f, "ydotool"),
        }
    }
}

/// A detected input backend with its binary path
#[derive(Debug, Clone)]
pub struct InputBackend {
    pub kind: InputBackendKind,
    pub binary_path: String,
}

/// Check if a command exists on PATH and return its path
fn which_tool(name: &str) -> Option<String> {
    let output = std::process::Command::new("which")
        .arg(name)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            None
        } else {
            Some(path)
        }
    } else {
        None
    }
}

/// Detect the best available input backend
pub fn detect_input_backend() -> Result<InputBackend, ComputerUseError> {
    use crate::capture::backend::{detect_display_server, DisplayServer};

    let ds = detect_display_server();
    info!("Detecting input backend for display server: {}", ds);

    match ds {
        DisplayServer::Wayland => {
            if let Some(path) = which_tool("ydotool") {
                info!("Using ydotool backend (Wayland) at {}", path);
                Ok(InputBackend {
                    kind: InputBackendKind::Ydotool,
                    binary_path: path,
                })
            } else {
                Err(ComputerUseError::NoInputBackendAvailable)
            }
        }
        DisplayServer::X11 => {
            if let Some(path) = which_tool("xdotool") {
                info!("Using xdotool backend (X11) at {}", path);
                Ok(InputBackend {
                    kind: InputBackendKind::Xdotool,
                    binary_path: path,
                })
            } else {
                Err(ComputerUseError::NoInputBackendAvailable)
            }
        }
        DisplayServer::Unknown => Err(ComputerUseError::NoDisplayServer),
    }
}

/// Get the display geometry via xdotool
pub async fn get_display_geometry(backend: &InputBackend) -> Result<(u32, u32), ComputerUseError> {
    use std::time::Duration;
    use tokio::process::Command;

    let mut cmd = Command::new(&backend.binary_path);
    cmd.arg("getdisplaygeometry")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = cmd.spawn().map_err(|e| {
        ComputerUseError::InputError(format!("Failed to spawn {}: {e}", backend.kind))
    })?;

    let output = tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
        .await
        .map_err(|_| ComputerUseError::InputTimeout { seconds: 5 })?
        .map_err(|e| ComputerUseError::InputError(format!("getdisplaygeometry failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ComputerUseError::InputError(format!(
            "getdisplaygeometry exited {}: {}",
            output.status, stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_display_geometry(&stdout)
}

/// Parse "3440 1440\n" into (3440, 1440)
fn parse_display_geometry(s: &str) -> Result<(u32, u32), ComputerUseError> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 2 {
        return Err(ComputerUseError::InputError(format!(
            "Unexpected getdisplaygeometry output: {s}"
        )));
    }
    let w = parts[0].parse::<u32>().map_err(|e| {
        ComputerUseError::InputError(format!("Failed to parse width '{}': {e}", parts[0]))
    })?;
    let h = parts[1].parse::<u32>().map_err(|e| {
        ComputerUseError::InputError(format!("Failed to parse height '{}': {e}", parts[1]))
    })?;
    Ok((w, h))
}

/// Parse "x:123 y:456 screen:0 window:12345" into (123, 456)
pub fn parse_mouse_location(s: &str) -> Result<(u32, u32), ComputerUseError> {
    let mut x: Option<u32> = None;
    let mut y: Option<u32> = None;

    for part in s.split_whitespace() {
        if let Some(val) = part.strip_prefix("x:") {
            x = Some(val.parse::<u32>().map_err(|e| {
                ComputerUseError::InputError(format!("Failed to parse x '{val}': {e}"))
            })?);
        } else if let Some(val) = part.strip_prefix("y:") {
            y = Some(val.parse::<u32>().map_err(|e| {
                ComputerUseError::InputError(format!("Failed to parse y '{val}': {e}"))
            })?);
        }
    }

    match (x, y) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(ComputerUseError::InputError(format!(
            "Could not parse mouse location from: {s}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Mutex to serialize tests that mutate display env vars.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_detect_xdotool() {
        let _lock = ENV_MUTEX.lock().expect("env mutex poisoned");
        // If xdotool is installed and we're on X11, detection should work
        let result = detect_input_backend();
        // We can't guarantee the result, but it shouldn't panic
        match result {
            Ok(backend) => {
                assert!(
                    backend.kind == InputBackendKind::Xdotool
                        || backend.kind == InputBackendKind::Ydotool
                );
                assert!(!backend.binary_path.is_empty());
            }
            Err(_) => {
                // Acceptable — no display or no tool
            }
        }
    }

    #[test]
    fn test_detect_no_backend() {
        let _lock = ENV_MUTEX.lock().expect("env mutex poisoned");
        let orig_wayland = std::env::var("WAYLAND_DISPLAY").ok();
        let orig_display = std::env::var("DISPLAY").ok();

        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("DISPLAY");

        let result = detect_input_backend();
        assert!(result.is_err());

        // Restore
        if let Some(v) = orig_wayland {
            std::env::set_var("WAYLAND_DISPLAY", v);
        }
        if let Some(v) = orig_display {
            std::env::set_var("DISPLAY", v);
        }
    }

    #[test]
    fn test_backend_kind_display() {
        assert_eq!(InputBackendKind::Xdotool.to_string(), "xdotool");
        assert_eq!(InputBackendKind::Ydotool.to_string(), "ydotool");
    }

    #[test]
    fn test_parse_display_geometry_valid() {
        let (w, h) = parse_display_geometry("3440 1440\n").expect("should parse");
        assert_eq!(w, 3440);
        assert_eq!(h, 1440);
    }

    #[test]
    fn test_parse_display_geometry_invalid() {
        assert!(parse_display_geometry("bad").is_err());
        assert!(parse_display_geometry("100").is_err());
        assert!(parse_display_geometry("abc def").is_err());
    }

    #[test]
    fn test_parse_mouse_location_valid() {
        let (x, y) =
            parse_mouse_location("x:123 y:456 screen:0 window:12345").expect("should parse");
        assert_eq!(x, 123);
        assert_eq!(y, 456);
    }

    #[test]
    fn test_parse_mouse_location_invalid() {
        assert!(parse_mouse_location("no coords here").is_err());
        assert!(parse_mouse_location("x:abc y:def").is_err());
    }
}
