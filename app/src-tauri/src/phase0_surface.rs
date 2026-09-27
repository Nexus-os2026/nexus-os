//! P0-002C5A: Phase Zero reachability closure.
//!
//! A desktop surface closed here had no approved backend-owned authority for
//! what it did: a raw frontend or model path, the process working directory,
//! a caller-asserted approval, arbitrary program text, or an ambient resource
//! location. Rather than inventing that authority, the surface fails closed.
//! An unavailable safe feature is preferred to a working unsafe one.
//!
//! Nothing here grants authority; it only denies. A closed IPC command stays
//! registered so the frontend receives one bounded, human-readable reason
//! instead of an unknown-command error. Its handler takes no arguments and its
//! whole body is the denial, so no caller input is read, echoed or acted on.
//!
//! Re-opening a surface requires an Architect-approved authority mechanism.
//! The reachability guard in `phase0_surface/tests.rs` fails when a closed
//! command, or a closed non-IPC path, becomes reachable again without that
//! classification; `docs/security/phase0-c5-authority-inventory.md` explains
//! how to update both.

/// Why a surface is unavailable in Phase Zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Closure {
    /// E1: a raw user-selected path is not authority, and no backend-owned
    /// selection mechanism exists in Phase Zero.
    FileSelection,
    /// E2: the retired raw-path Builder surface. The governed Builder project
    /// flow (C1-C4) is the approved authority path.
    LegacyBuilder,
}

impl Closure {
    pub(crate) const fn reason(self) -> &'static str {
        match self {
            Self::FileSelection => {
                "governed file selection is unavailable in Phase Zero; a raw file path is not authority"
            }
            Self::LegacyBuilder => {
                "the legacy Builder project surface is retired in Phase Zero; use the governed Builder project flow"
            }
        }
    }
}

/// The bounded error for a closed surface: `"<surface>: <reason>"`.
pub(crate) fn closed(surface: &'static str, closure: Closure) -> String {
    format!("{surface}: {}", closure.reason())
}

#[cfg(test)]
mod tests;
