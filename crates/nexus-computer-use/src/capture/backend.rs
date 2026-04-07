use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::capture::CaptureRegion;
use crate::error::ComputerUseError;

const CAPTURE_TIMEOUT_SECS: u64 = 10;

/// Supported capture backends
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Wayland — preferred
    Grim,
    /// X11 — primary
    Scrot,
    /// X11 — fallback (ImageMagick `import`)
    XcbCapture,
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendKind::Grim => write!(f, "grim"),
            BackendKind::Scrot => write!(f, "scrot"),
            BackendKind::XcbCapture => write!(f, "import"),
        }
    }
}

/// Detected display server
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayServer {
    Wayland,
    X11,
    Unknown,
}

impl std::fmt::Display for DisplayServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayServer::Wayland => write!(f, "wayland"),
            DisplayServer::X11 => write!(f, "x11"),
            DisplayServer::Unknown => write!(f, "unknown"),
        }
    }
}

/// A detected capture backend with its display server
#[derive(Debug)]
pub struct CaptureBackend {
    pub kind: BackendKind,
    pub display_server: DisplayServer,
}

/// Detect the current display server from environment variables
pub fn detect_display_server() -> DisplayServer {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        DisplayServer::Wayland
    } else if std::env::var("DISPLAY").is_ok() {
        DisplayServer::X11
    } else {
        DisplayServer::Unknown
    }
}

/// Check if a command exists on PATH
fn command_exists(name: &str) -> bool {
    which_sync(name)
}

fn which_sync(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Detect the best available capture backend
pub fn detect_backend() -> Result<CaptureBackend, ComputerUseError> {
    let ds = detect_display_server();
    info!("Detected display server: {}", ds);

    match ds {
        DisplayServer::Wayland => {
            if command_exists("grim") {
                info!("Using grim backend (Wayland)");
                Ok(CaptureBackend {
                    kind: BackendKind::Grim,
                    display_server: ds,
                })
            } else {
                warn!("Wayland detected but grim not found");
                Err(ComputerUseError::NoBackendAvailable)
            }
        }
        DisplayServer::X11 => {
            if command_exists("scrot") {
                info!("Using scrot backend (X11)");
                Ok(CaptureBackend {
                    kind: BackendKind::Scrot,
                    display_server: ds,
                })
            } else if command_exists("import") {
                info!("Using import backend (X11, ImageMagick)");
                Ok(CaptureBackend {
                    kind: BackendKind::XcbCapture,
                    display_server: ds,
                })
            } else {
                warn!("X11 detected but no capture tool found");
                Err(ComputerUseError::NoBackendAvailable)
            }
        }
        DisplayServer::Unknown => {
            debug!("No display server detected");
            Err(ComputerUseError::NoDisplayServer)
        }
    }
}

impl CaptureBackend {
    /// Execute a screen capture, writing the result to `output_path`
    pub async fn capture(
        &self,
        output_path: &Path,
        region: Option<&CaptureRegion>,
    ) -> Result<(), ComputerUseError> {
        let output_str = output_path
            .to_str()
            .ok_or_else(|| ComputerUseError::CaptureError("Invalid output path".into()))?;

        let mut cmd = match (&self.kind, region) {
            (BackendKind::Grim, None) => {
                let mut c = Command::new("grim");
                c.arg("-t").arg("png").arg(output_str);
                c
            }
            (BackendKind::Grim, Some(r)) => {
                let mut c = Command::new("grim");
                c.arg("-g")
                    .arg(format!("{},{} {}x{}", r.x, r.y, r.width, r.height))
                    .arg("-t")
                    .arg("png")
                    .arg(output_str);
                c
            }
            (BackendKind::Scrot, None) => {
                let mut c = Command::new("scrot");
                c.arg("-o").arg(output_str);
                c
            }
            (BackendKind::Scrot, Some(r)) => {
                let mut c = Command::new("scrot");
                c.arg("-a")
                    .arg(format!("{},{},{},{}", r.x, r.y, r.width, r.height))
                    .arg("-o")
                    .arg(output_str);
                c
            }
            (BackendKind::XcbCapture, None) => {
                let mut c = Command::new("import");
                c.arg("-window").arg("root").arg(output_str);
                c
            }
            (BackendKind::XcbCapture, Some(_)) => {
                // import doesn't support region natively in a simple way,
                // capture full screen and let the caller crop via image crate
                let mut c = Command::new("import");
                c.arg("-window").arg("root").arg(output_str);
                c
            }
        };

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        debug!("Running capture command: {:?}", cmd);

        let child = cmd.spawn().map_err(|e| {
            ComputerUseError::CaptureError(format!("Failed to spawn capture process: {e}"))
        })?;

        let output = tokio::time::timeout(
            Duration::from_secs(CAPTURE_TIMEOUT_SECS),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| ComputerUseError::Timeout {
            seconds: CAPTURE_TIMEOUT_SECS,
        })?
        .map_err(|e| ComputerUseError::CaptureError(format!("Capture process failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ComputerUseError::CaptureError(format!(
                "{} exited with {}: {}",
                self.kind, output.status, stderr
            )));
        }

        // Verify the output file exists
        if !output_path.exists() {
            return Err(ComputerUseError::CaptureError(
                "Capture completed but output file not found".into(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Mutex to serialize all tests that mutate WAYLAND_DISPLAY / DISPLAY env vars.
    /// Without this, parallel test threads race on the shared process environment.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    /// Save both display env vars, run a closure, then restore them.
    fn with_display_env<F: FnOnce()>(wayland: Option<&str>, display: Option<&str>, f: F) {
        let _lock = ENV_MUTEX.lock().expect("env mutex poisoned");

        let orig_wayland = std::env::var("WAYLAND_DISPLAY").ok();
        let orig_display = std::env::var("DISPLAY").ok();

        // Set the desired state
        match wayland {
            Some(v) => std::env::set_var("WAYLAND_DISPLAY", v),
            None => std::env::remove_var("WAYLAND_DISPLAY"),
        }
        match display {
            Some(v) => std::env::set_var("DISPLAY", v),
            None => std::env::remove_var("DISPLAY"),
        }

        f();

        // Restore
        match orig_wayland {
            Some(v) => std::env::set_var("WAYLAND_DISPLAY", v),
            None => std::env::remove_var("WAYLAND_DISPLAY"),
        }
        match orig_display {
            Some(v) => std::env::set_var("DISPLAY", v),
            None => std::env::remove_var("DISPLAY"),
        }
    }

    #[test]
    fn test_detect_wayland() {
        with_display_env(Some("wayland-0"), None, || {
            assert_eq!(detect_display_server(), DisplayServer::Wayland);
        });
    }

    #[test]
    fn test_detect_x11() {
        with_display_env(None, Some(":0"), || {
            assert_eq!(detect_display_server(), DisplayServer::X11);
        });
    }

    #[test]
    fn test_detect_no_display() {
        with_display_env(None, None, || {
            assert_eq!(detect_display_server(), DisplayServer::Unknown);
        });
    }

    #[test]
    fn test_detect_display_server_wayland_priority() {
        with_display_env(Some("wayland-0"), Some(":0"), || {
            assert_eq!(detect_display_server(), DisplayServer::Wayland);
        });
    }

    #[test]
    fn test_backend_kind_display() {
        assert_eq!(BackendKind::Grim.to_string(), "grim");
        assert_eq!(BackendKind::Scrot.to_string(), "scrot");
        assert_eq!(BackendKind::XcbCapture.to_string(), "import");
    }

    #[test]
    fn test_display_server_display() {
        assert_eq!(DisplayServer::Wayland.to_string(), "wayland");
        assert_eq!(DisplayServer::X11.to_string(), "x11");
        assert_eq!(DisplayServer::Unknown.to_string(), "unknown");
    }

    #[test]
    fn test_no_backend_when_no_display() {
        with_display_env(None, None, || {
            let result = detect_backend();
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(
                matches!(err, ComputerUseError::NoDisplayServer),
                "Expected NoDisplayServer, got: {err}"
            );
        });
    }

    #[test]
    fn test_backend_kind_equality() {
        assert_eq!(BackendKind::Grim, BackendKind::Grim);
        assert_ne!(BackendKind::Grim, BackendKind::Scrot);
        assert_ne!(BackendKind::Scrot, BackendKind::XcbCapture);
    }
}
