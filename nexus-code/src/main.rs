//! `nx` (`nexus-code/src/main.rs`), the standalone Nexus Code terminal, is
//! withdrawn during Phase Zero.
//!
//! This executable used to run the Nexus Code agent outside the governed
//! desktop, in the directory it was started from (Final-Gate dossier item
//! J5): before any command it read `NEXUSCODE.md` and `.nxrc` from the working
//! directory (`.nxrc` could auto-approve tools such as `bash`) and the
//! platform configuration directory, and probed for and preferred the external
//! `claude` CLI; its tools, MCP servers (`--mcp-config`) and computer use
//! (`--computer-use`) ran rooted at the working directory, with approval
//! switches for headless runs, and `init` and `bench` wrote into it. Every
//! invocation now writes one fixed message to standard error and exits with
//! one fixed non-zero status.
//!
//! Before that denial it reads no argument, environment variable,
//! configuration or credential, touches no file, and starts no runtime,
//! socket or process. No command, flag or environment variable restores it.
//! The `nexus_code` library is unchanged: the desktop keeps using its
//! `_for_desktop` entry points, which read no project files. The library's
//! standalone entry points are not claimed governed.
//!
//! The withdrawal contract is tested against the built executable
//! (`tests/phase0_withdrawal.rs`). The unit-test build of this target has no
//! entry point, so only the real executable carries the withdrawal message.

#![forbid(unsafe_code)]

#[cfg(not(test))]
fn main() -> std::process::ExitCode {
    use std::io::Write;

    const WITHDRAWN_MESSAGE: &str = "nx: unavailable during Phase Zero; standalone use withdrawn";
    // sysexits EX_UNAVAILABLE.
    const WITHDRAWN_STATUS: u8 = 69;

    // A failed write changes nothing: the status is the same either way.
    let _ = writeln!(std::io::stderr(), "{WITHDRAWN_MESSAGE}");
    std::process::ExitCode::from(WITHDRAWN_STATUS)
}
