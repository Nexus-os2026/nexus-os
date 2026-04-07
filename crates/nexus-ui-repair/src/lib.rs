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
//! **Still gated to Phase 1.3.5:** Xvfb isolation, real input events,
//! real screen capture, and a live DOM path for the enumerator. The
//! driver loop remains a Phase 1.2 stub.
//!
//! This crate is **read-only by design.** It cannot modify Nexus OS
//! source code. Repairs are performed interactively by a human +
//! Claude Code in Phase B.

use std::path::PathBuf;

pub mod descriptors;
pub mod driver;
pub mod governance;
pub mod ledger;
pub mod replay;
pub mod specialists;

/// Crate version, mirrored for convenience.
pub const VERSION: &str = "0.1.0";

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
