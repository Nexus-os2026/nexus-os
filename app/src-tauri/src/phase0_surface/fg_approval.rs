//! P0-FINAL-GATE-CLOSURE guards for item G (approval channels) and C5
//! (the residual capability-measurement route).
//!
//! The guards are in `fg_approval/tests.rs`. They read desktop, kernel and
//! crate sources as data; a file named `tests.rs` is test-only by the
//! convention of the desktop's source guards, so those names are not read as
//! desktop production code.

mod tests;
