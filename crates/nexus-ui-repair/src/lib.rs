//! # nexus-ui-repair — autonomous QA scout
//!
//! Crate #69. Implements the read-only "scout" half of the
//! NEXUS_UI_REPAIR v1.1 design (see
//! `docs/roadmaps/NEXUS_UI_REPAIR_v1.1_scout_repair_split.md`).
//!
//! **Phase 1.3 status:** structural gates landed for Hole A Layer 2
//! (per-app input governance via [`governance::InputSandbox`]) and
//! Hole B Layers 2+3 (modal handler via
//! [`specialists::modal_handler::ModalHandler`] and page-descriptor
//! opt-in validation via [`descriptors::PageDescriptor::validate`]).
//! The vision-judge calibration log ([`governance::CalibrationLog`])
//! and the I-5 output-capture seam ([`specialists::SpecialistCall`]
//! plus [`governance::audit::AuditLog::record_specialist_call`]) are
//! also wired. The crate now imports `nexus-computer-use` for
//! governance TYPES ONLY — no screen capture or input event code
//! path is exercised in Phase 1.3.
//!
//! **Phase 1.3.5 status (this commit):** the scout now exercises real
//! capture and input code paths via `nexus-computer-use`:
//!
//! - [`governance::XvfbSession`] — Hole A Layer 3 structural Xvfb
//!   isolation. Owns a child Xvfb process on a unique display number
//!   in `99..150` and tears it down on Drop.
//! - [`specialists::EyesAndHands`] — first specialist that calls real
//!   `nexus_computer_use::capture::take_screenshot` and
//!   `nexus_computer_use::input::MouseController::execute`. Sync façade
//!   over the async crate API via a per-call current-thread tokio
//!   runtime.
//! - [`governance::InputSandbox::validate_and_click`] — Hole A Layer 2
//!   ACTIVE. Validates the target window through the real
//!   `AppGrantManager` denial path before issuing any input event.
//! - `tests/xvfb_smoke.rs` — two `#[ignore]`'d **structural** tests
//!   that verify the XvfbSession + EyesAndHands wiring spawns,
//!   captures, and drives input without panicking. They do NOT
//!   assert pixel-level correctness: bare Xvfb has no painted
//!   software cursor, no window manager motion delivery, and we
//!   observed an X server quirk where `xsetroot -solid` changes
//!   read back as byte-identical PNGs via scrot. Real end-to-end
//!   pixel verification is deferred to **Phase 1.5.5**, which will
//!   run a real Nexus OS Tauri WebView inside the same XvfbSession
//!   and assert against actual framebuffer damage events.
//!
//! **Phase 1.4 (next-next):** wire `EyesAndHands` into the driver loop
//! and layer the `vision_judge` LLM on top of `CaptureResult`. The
//! driver loop and the live DOM path for the enumerator remain Phase
//! 1.2 stubs until then.
//!
//! This crate is **read-only by design.** It cannot modify Nexus OS
//! source code. Repairs are performed interactively by a human +
//! Claude Code in Phase B.

use std::path::PathBuf;

pub mod comparison;
pub mod descriptors;
pub mod driver;
pub mod governance;
pub mod ground_truth;
pub mod ledger;
pub mod repair_ticket;
pub mod replay;
pub mod specialists;

/// Crate version, mirrored for convenience.
///
/// Phase 1.4 bumped the mirrored version constant to `0.4.0`. The
/// Cargo.toml `version` field still inherits `workspace.package.version`
/// because the scout ships inside the Nexus OS workspace and must track
/// the workspace release cadence — see Phase 1.4 ship notes.
pub const VERSION: &str = "0.4.0";

/// Top-level error type for `nexus-ui-repair`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// One of the five v1.1 invariants (I-1..I-5) was violated.
    #[error("invariant violation: {0}")]
    InvariantViolation(String),

    /// A write was attempted outside the ACL allowlist (I-2 enforcement).
    #[error("ACL denied write to {0}")]
    AclDenied(PathBuf),

    /// A provider not in the v1.1 §4 allowed routing table was requested.
    #[error("provider forbidden by routing table: {0}")]
    ProviderForbidden(String),

    /// Underlying I/O failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON (de)serialization failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Convenience alias for `Result<T, crate::Error>`.
pub type Result<T> = std::result::Result<T, Error>;
