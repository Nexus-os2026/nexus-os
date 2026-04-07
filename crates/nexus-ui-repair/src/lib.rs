//! # nexus-ui-repair — autonomous QA scout
//!
//! Crate #69. Implements the read-only "scout" half of the
//! NEXUS_UI_REPAIR v1.1 design (see
//! `docs/roadmaps/NEXUS_UI_REPAIR_v1.1_scout_repair_split.md`).
//!
//! **Phase 1.1 status:** skeleton only. Every module compiles to a stub
//! and passes one trivial test, with the single exception of
//! `tests/acl.rs` which exercises invariant I-2 for real. No specialists
//! are wired, no `nexus-computer-use` integration, no `nexus-memory`
//! coupling, no provider calls. Phase 1.2 begins wiring real behavior.
//!
//! This crate is **read-only by design.** It cannot modify Nexus OS
//! source code. Repairs are performed interactively by a human +
//! Claude Code in Phase B.

use std::path::PathBuf;

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
