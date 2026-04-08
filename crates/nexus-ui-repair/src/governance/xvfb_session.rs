//! Hole A Layer 3 enforcement: Xvfb display isolation for the scout's
//! real input and capture pipeline. Each `XvfbSession` owns a child
//! Xvfb process and a unique display number. The scout's input/capture
//! calls set `DISPLAY` in the process env, and shell-tool backends
//! (scrot, xdotool) automatically route to the isolated display.
//!
//! Phase 1.3.5 ships this as the **first active use** of the sandbox.
//! Real input events cross this boundary into a real X server (just
//! one that's headless and isolated). Phase 1.5.5 will swap the test
//! target from xeyes to a real Nexus OS instance running inside the
//! same Xvfb display.

use crate::Error;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

/// A spawned, isolated Xvfb display owned by this process.
///
/// Drops the Xvfb child on `Drop` (best-effort kill + wait).
pub struct XvfbSession {
    child: Child,
    display_num: u32,
}

impl XvfbSession {
    /// Spawn a new Xvfb instance on the first available display
    /// number in the range 99..150. Waits up to 5 seconds for the
    /// display to become ready (detected via `/tmp/.X{N}-lock`).
    pub fn spawn() -> crate::Result<Self> {
        for display_num in 99u32..150 {
            let lock_path = PathBuf::from(format!("/tmp/.X{display_num}-lock"));
            if lock_path.exists() {
                continue;
            }

            let display_arg = format!(":{display_num}");
            let result = Command::new("Xvfb")
                .arg(&display_arg)
                .arg("-screen")
                .arg("0")
                .arg("1024x768x24")
                .arg("-ac")
                .arg("-nolisten")
                .arg("tcp")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();

            let child = match result {
                Ok(c) => c,
                Err(e) => return Err(Error::Io(e)),
            };

            let session = Self { child, display_num };

            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if lock_path.exists() {
                    return Ok(session);
                }
                std::thread::sleep(Duration::from_millis(50));
            }

            // Timed out — drop kills the child. Surface as invariant
            // violation; do not silently try the next display, since
            // a hung Xvfb usually means a system-level issue.
            drop(session);
            return Err(Error::InvariantViolation(format!(
                "XvfbSession: Xvfb on :{display_num} did not become ready within 5s"
            )));
        }

        Err(Error::InvariantViolation(
            "XvfbSession: no available display number in range 99..150".into(),
        ))
    }

    /// Display string (e.g. `":99"`) suitable for the `DISPLAY` env var.
    pub fn display(&self) -> String {
        format!(":{}", self.display_num)
    }

    /// Numeric display id.
    pub fn display_num(&self) -> u32 {
        self.display_num
    }
}

impl Drop for XvfbSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
