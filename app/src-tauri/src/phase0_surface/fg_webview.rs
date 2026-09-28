//! P0-FINAL-GATE-CLOSURE guards for item D (privileged webview, navigation and
//! IPC origin).
//!
//! The guard code lives in the `tests` submodule (`fg_webview/tests.rs`): its
//! name ends in `tests.rs`, so the `phase0_surface` production scanners skip it
//! and it may name guarded spellings (command names, manifest-directory
//! macros, capability identifiers) without tripping other guards. This module
//! is already `#[cfg(test)]` through its declaration in `phase0_surface.rs`.

mod tests;
