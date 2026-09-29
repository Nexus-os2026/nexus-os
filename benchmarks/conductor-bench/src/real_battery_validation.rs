//! `real-battery-validation` (`src/real_battery_validation.rs`) is withdrawn
//! during Phase Zero.
//!
//! This benchmark read `GROQ_API_KEY` (its usage told users to put an NVIDIA
//! `nvapi-` key there) and, through the `NimClient` of
//! `nexus-capability-measurement`, sent that value as a bearer token to Groq
//! (`api.groq.com`) in an `authorization` header on curl's command line,
//! where any local process could read it; an NVIDIA key set as its usage said
//! went to Groq. It loaded the battery from
//! `crates/nexus-capability-measurement/data/battery_v1.json` and agent
//! manifests from `agents/prebuilt` under the working directory, and saved
//! the run under `data/validation_runs` there. Every invocation now writes one
//! fixed message to standard error and exits with one fixed non-zero status.
//!
//! Before that denial it reads no argument, environment variable, key or
//! file, contacts no provider, writes no result and starts no process. No
//! flag or environment variable restores it. The library it used
//! (`nexus-capability-measurement`) is unchanged and is not claimed governed,
//! and the other benchmark binaries of this package are unchanged.
//!
//! The withdrawal contract is tested against the built executable
//! (`tests/phase0_withdrawal.rs`). The unit-test build of this target has no
//! entry point, so only the real executable carries the withdrawal message.

#![forbid(unsafe_code)]

#[cfg(not(test))]
fn main() -> std::process::ExitCode {
    use std::io::Write;

    const WITHDRAWN_MESSAGE: &str =
        "real-battery-validation: unavailable during Phase Zero; standalone use withdrawn";
    // sysexits EX_UNAVAILABLE.
    const WITHDRAWN_STATUS: u8 = 69;

    // A failed write changes nothing: the status is the same either way.
    let _ = writeln!(std::io::stderr(), "{WITHDRAWN_MESSAGE}");
    std::process::ExitCode::from(WITHDRAWN_STATUS)
}
