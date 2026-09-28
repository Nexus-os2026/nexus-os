//! `nexus-cli` (`cli/src/main.rs`, the `nexus` command) is withdrawn during
//! Phase Zero.
//!
//! This executable used to run every `nexus` command outside the governed
//! desktop, with the invoking user's authority, in the directory it was
//! started from (Final-Gate dossier item J4): `voice` ran `python3 jarvis.py`
//! from the working directory's `voice/` (or the build checkout), `agent start`
//! ran the coding agent's shell test command and the social poster (live
//! publishing unless `--dry-run`) from manifests found in the working
//! directory, `conduct` wrote into the working directory, `setup` rewrote the
//! Nexus configuration and `marketplace` changed the shared agent registry.
//! Every invocation now writes one fixed message to standard error and exits
//! with one fixed non-zero status.
//!
//! Before that denial it reads no argument, environment variable,
//! configuration or credential, touches no file, and starts no runtime,
//! socket or process. No command, flag or environment variable restores it;
//! it is withdrawn whole. The `nexus_cli` library is unchanged and is not
//! claimed governed.
//!
//! The withdrawal contract is tested against the built executable
//! (`tests/phase0_withdrawal.rs`). The unit-test build of this target has no
//! entry point, so only the real executable carries the withdrawal message.

#![forbid(unsafe_code)]

#[cfg(not(test))]
fn main() -> std::process::ExitCode {
    use std::io::Write;

    const WITHDRAWN_MESSAGE: &str =
        "nexus-cli: unavailable during Phase Zero; standalone use withdrawn";
    // sysexits EX_UNAVAILABLE.
    const WITHDRAWN_STATUS: u8 = 69;

    // A failed write changes nothing: the status is the same either way.
    let _ = writeln!(std::io::stderr(), "{WITHDRAWN_MESSAGE}");
    std::process::ExitCode::from(WITHDRAWN_STATUS)
}
