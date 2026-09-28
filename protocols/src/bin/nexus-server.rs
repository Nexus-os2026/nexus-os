//! `nexus-protocols-server` (`protocols/src/bin/nexus-server.rs`) is withdrawn
//! during Phase Zero.
//!
//! This executable used to run the protocols HTTP gateway
//! (`nexus_protocols::server_runtime`) on `NEXUS_HTTP_ADDR`, by default every
//! interface on port 8080: A2A, MCP tool invocation over the kernel's default
//! actuator registry, OpenAI- and Anthropic-compatible endpoints, a WebSocket
//! event stream and a frontend served from the working directory (Final-Gate
//! dossier item J2). Every invocation now writes one fixed message to
//! standard error and exits with one fixed non-zero status.
//!
//! Before that denial it reads no argument, environment variable,
//! configuration or credential, touches no file, starts no runtime, binds no
//! socket and starts no process. No flag, argument or environment variable
//! restores the server. The `nexus_protocols` library is unchanged.
//!
//! The withdrawal contract is tested against the built executable
//! (`tests/phase0_withdrawal.rs`). The unit-test build of this target has no
//! entry point, so only the real executable carries the withdrawal message.

#![forbid(unsafe_code)]

#[cfg(not(test))]
fn main() -> std::process::ExitCode {
    use std::io::Write;

    const WITHDRAWN_MESSAGE: &str =
        "nexus-protocols-server: unavailable during Phase Zero; deployment withdrawn";
    // sysexits EX_UNAVAILABLE.
    const WITHDRAWN_STATUS: u8 = 69;

    // A failed write changes nothing: the status is the same either way.
    let _ = writeln!(std::io::stderr(), "{WITHDRAWN_MESSAGE}");
    std::process::ExitCode::from(WITHDRAWN_STATUS)
}
