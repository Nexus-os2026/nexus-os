//! `nx-govern` (`src/bin/governance_test.rs`) is withdrawn during Phase Zero.
//!
//! This harness listed and inspected the visible and focused applications,
//! granted computer-use access levels to application classes, and tried
//! clicks and typing against the focused application through the platform
//! input tools, outside the governed desktop. Every invocation now writes one
//! fixed message to standard error and exits with one fixed non-zero status.
//!
//! Before that denial it reads no argument, environment variable or file,
//! inspects no window, grants nothing, sends no input and starts no process.
//! No flag or environment variable restores it. The `nexus_computer_use`
//! library is unchanged and is not claimed governed.
//!
//! The withdrawal contract is tested against the built executable
//! (`tests/phase0_withdrawal.rs`). The unit-test build of this target has no
//! entry point, so only the real executable carries the withdrawal message.

#![forbid(unsafe_code)]

#[cfg(not(test))]
fn main() -> std::process::ExitCode {
    use std::io::Write;

    const WITHDRAWN_MESSAGE: &str =
        "nx-govern: unavailable during Phase Zero; standalone use withdrawn";
    // sysexits EX_UNAVAILABLE.
    const WITHDRAWN_STATUS: u8 = 69;

    // A failed write changes nothing: the status is the same either way.
    let _ = writeln!(std::io::stderr(), "{WITHDRAWN_MESSAGE}");
    std::process::ExitCode::from(WITHDRAWN_STATUS)
}
