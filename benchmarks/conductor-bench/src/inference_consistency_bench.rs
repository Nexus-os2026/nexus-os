//! `inference-consistency-bench` (`src/inference_consistency_bench.rs`) is
//! withdrawn during Phase Zero.
//!
//! This benchmark read `GROQ_API_KEY` and sent that value as a bearer token
//! to NVIDIA NIM (`integrate.api.nvidia.com`) in an `authorization` header on
//! curl's command line, where any local process could read it; a Groq key set
//! for Groq went to NVIDIA. It also queried the Ollama service at
//! `OLLAMA_URL` with up to 1000 concurrent requests (optionally for an hour)
//! and wrote `INFERENCE_CONSISTENCY_RESULTS.md` into the working directory.
//! Every invocation now writes one fixed message to standard error and exits
//! with one fixed non-zero status.
//!
//! Before that denial it reads no argument, environment variable, key or
//! file, contacts no provider or local service, writes no report and starts
//! no process. No flag or environment variable restores it. The library it
//! used (`nexus-kernel`) is unchanged and is not claimed governed, and the
//! other benchmark binaries of this package are unchanged.
//!
//! The withdrawal contract is tested against the built executable
//! (`tests/phase0_withdrawal.rs`). The unit-test build of this target has no
//! entry point, so only the real executable carries the withdrawal message.

#![forbid(unsafe_code)]

#[cfg(not(test))]
fn main() -> std::process::ExitCode {
    use std::io::Write;

    const WITHDRAWN_MESSAGE: &str =
        "inference-consistency-bench: unavailable during Phase Zero; standalone use withdrawn";
    // sysexits EX_UNAVAILABLE.
    const WITHDRAWN_STATUS: u8 = 69;

    // A failed write changes nothing: the status is the same either way.
    let _ = writeln!(std::io::stderr(), "{WITHDRAWN_MESSAGE}");
    std::process::ExitCode::from(WITHDRAWN_STATUS)
}
