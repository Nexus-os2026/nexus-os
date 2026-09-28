//! `cloud-models-bench` (`src/cloud_models_bench.rs`) is withdrawn during
//! Phase Zero.
//!
//! This benchmark read up to ten provider keys from the environment
//! (DeepSeek, Groq, Mistral, Together, Fireworks, Perplexity, OpenRouter,
//! OpenAI, Gemini, Cohere) and sent each as a bearer token in an
//! `authorization` header on curl's command line, where any local process
//! could read it; its NVIDIA NIM pass sent the `GROQ_API_KEY` value to
//! `integrate.api.nvidia.com`, so a Groq key also went to NVIDIA. It also
//! queried the Ollama service at `OLLAMA_URL` and wrote
//! `CLOUD_MODELS_COMPARISON_RESULTS.md` into the working directory. Every
//! invocation now writes one fixed message to standard error and exits with
//! one fixed non-zero status.
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
        "cloud-models-bench: unavailable during Phase Zero; standalone use withdrawn";
    // sysexits EX_UNAVAILABLE.
    const WITHDRAWN_STATUS: u8 = 69;

    // A failed write changes nothing: the status is the same either way.
    let _ = writeln!(std::io::stderr(), "{WITHDRAWN_MESSAGE}");
    std::process::ExitCode::from(WITHDRAWN_STATUS)
}
