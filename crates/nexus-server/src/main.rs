//! `nexus-server` (`crates/nexus-server`) is withdrawn during Phase Zero.
//!
//! This executable used to serve an HTTP API, the `nexus-mcp` tools and an
//! A2A endpoint on every network interface, with no authentication and a
//! permissive CORS policy (Final-Gate dossier item J1). P0-FG1 withdraws it:
//! every invocation writes one fixed message to standard error and exits with
//! one fixed non-zero status.
//!
//! Before that denial it reads no argument, environment variable,
//! configuration or credential, touches no file, starts no runtime, binds no
//! socket, builds no MCP server and starts no process. No flag, argument or
//! environment variable restores the server.
//!
//! The withdrawal contract is tested against the built executable
//! (`tests/phase0_withdrawal.rs`). The unit-test build of this target has no
//! entry point, so only the real executable carries the withdrawal message.

#![forbid(unsafe_code)]

#[cfg(not(test))]
fn main() -> std::process::ExitCode {
    use std::io::Write;

    const WITHDRAWN_MESSAGE: &str =
        "nexus-server: unavailable during Phase Zero; deployment withdrawn";
    // sysexits EX_UNAVAILABLE.
    const WITHDRAWN_STATUS: u8 = 69;

    // A failed write changes nothing: the status is the same either way.
    let _ = writeln!(std::io::stderr(), "{WITHDRAWN_MESSAGE}");
    std::process::ExitCode::from(WITHDRAWN_STATUS)
}
