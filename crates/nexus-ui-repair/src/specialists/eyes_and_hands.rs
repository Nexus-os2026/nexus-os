//! Phase 1.3.5 — first specialist that calls real `nexus-computer-use`
//! capture and input functions.
//!
//! `EyesAndHands` is a thin sync wrapper around the async
//! `nexus_computer_use::capture::take_screenshot` and
//! `nexus_computer_use::input::MouseController::execute` APIs. Each call
//! constructs a per-call current-thread tokio runtime and `block_on`s
//! the underlying async function — the scout's driver loop is not yet
//! async (Phase 1.4 will revisit), and the smoke test contract requires
//! a sync interface.
//!
//! All calls read `DISPLAY` from the process environment via the
//! underlying backend auto-detection (`detect_input_backend` /
//! capture backend probe). Callers must set `DISPLAY` themselves —
//! typically via [`crate::governance::XvfbSession::display`].
//!
//! Phase 1.4 will add `vision_judge` LLM integration on top of the
//! `CaptureResult` returned here.

use crate::Error;
use nexus_computer_use::capture::screenshot::take_screenshot;
use nexus_computer_use::capture::ScreenshotOptions;
use nexus_computer_use::input::{detect_input_backend, MouseAction, MouseButton, MouseController};

/// Result of a screen capture, normalized for the scout's pipeline.
pub struct CaptureResult {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub hash: String,
}

/// Real input + capture facade. Stateless — construct freely.
pub struct EyesAndHands;

impl Default for EyesAndHands {
    fn default() -> Self {
        Self::new()
    }
}

impl EyesAndHands {
    pub fn new() -> Self {
        Self
    }

    /// Capture the current display. Reads `DISPLAY` from process env.
    pub fn capture(&self) -> crate::Result<CaptureResult> {
        let shot = block_on(async { take_screenshot(ScreenshotOptions::default()).await })
            .map_err(|e| Error::InvariantViolation(format!("capture failed: {e}")))?;
        Ok(CaptureResult {
            bytes: shot.png_bytes,
            width: shot.width,
            height: shot.height,
            hash: shot.audit_hash,
        })
    }

    /// Click at `(x, y)` on the current display with the left button.
    pub fn click(&self, x: u32, y: u32) -> crate::Result<()> {
        self.execute_mouse(MouseAction::Click {
            x,
            y,
            button: MouseButton::Left,
        })
        .map(|_| ())
    }

    /// Move the cursor to `(x, y)` without clicking.
    pub fn move_cursor(&self, x: u32, y: u32) -> crate::Result<()> {
        self.execute_mouse(MouseAction::Move { x, y }).map(|_| ())
    }

    /// Return the current cursor position as `(x, y)`.
    pub fn cursor_position(&self) -> crate::Result<(u32, u32)> {
        let result = self.execute_mouse(MouseAction::GetPosition)?;
        result.position.ok_or_else(|| {
            Error::InvariantViolation("cursor_position: backend returned no position".into())
        })
    }

    fn execute_mouse(
        &self,
        action: MouseAction,
    ) -> crate::Result<nexus_computer_use::input::MouseActionResult> {
        let backend = detect_input_backend()
            .map_err(|e| Error::InvariantViolation(format!("input backend detect failed: {e}")))?;
        let controller = MouseController::new(backend);
        block_on(async move { controller.execute(&action).await })
            .map_err(|e| Error::InvariantViolation(format!("mouse action failed: {e}")))
    }
}

/// Run an async future to completion on a fresh current-thread runtime.
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build current-thread tokio runtime")
        .block_on(fut)
}
