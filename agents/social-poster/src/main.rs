//! `social-poster-agent` (`agents/social-poster/src/main.rs`) is withdrawn
//! during Phase Zero.
//!
//! This executable was an alternate entry point to the withdrawn `nexus-cli`
//! command `agent start social-poster` (Final-Gate dossier item J4): it loaded
//! a manifest from the working directory (or the build checkout) and ran the
//! social poster, which researches, generates and, unless `--dry-run` is
//! given, publishes posts with the configured credentials. Every invocation
//! now writes one fixed message to standard error and exits with one fixed
//! non-zero status.
//!
//! Before that denial it reads no argument, environment variable, manifest,
//! credential or file and starts no process or connection. No flag or
//! environment variable restores it. The `social_poster_agent` library is
//! unchanged and is not claimed governed.
//!
//! The withdrawal contract is tested against the built executable
//! (`tests/phase0_withdrawal.rs`). The unit-test build of this target has no
//! entry point, so only the real executable carries the withdrawal message.

#![forbid(unsafe_code)]

#[cfg(not(test))]
fn main() -> std::process::ExitCode {
    use std::io::Write;

    const WITHDRAWN_MESSAGE: &str =
        "social-poster-agent: unavailable during Phase Zero; standalone use withdrawn";
    // sysexits EX_UNAVAILABLE.
    const WITHDRAWN_STATUS: u8 = 69;

    // A failed write changes nothing: the status is the same either way.
    let _ = writeln!(std::io::stderr(), "{WITHDRAWN_MESSAGE}");
    std::process::ExitCode::from(WITHDRAWN_STATUS)
}
