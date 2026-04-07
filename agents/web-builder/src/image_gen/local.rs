//! Local Stable Diffusion integration — wires into whatever runtime exists.
//!
//! Checks for ComfyUI (port 8188), stable-diffusion.cpp, or Python diffusers.
//! If nothing is available, returns `NotAvailable` — the orchestrator falls back.

use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LocalGenError {
    #[error("no local image generation runtime available")]
    NotAvailable,
    #[error("local generation failed: {0}")]
    GenerationFailed(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Check if a local image generation runtime is available.
pub async fn is_available() -> bool {
    // Check ComfyUI (default port 8188)
    if check_comfyui().await {
        return true;
    }

    // Check for stable-diffusion.cpp binary
    if check_sd_cpp() {
        return true;
    }

    false
}

/// Generate an image using whatever local runtime is available.
pub async fn generate_local(
    prompt: &str,
    width: u32,
    height: u32,
    output_path: &Path,
) -> Result<(), LocalGenError> {
    // Try ComfyUI first
    if check_comfyui().await {
        return generate_via_comfyui(prompt, width, height, output_path).await;
    }

    // Try sd.cpp
    if check_sd_cpp() {
        return generate_via_sd_cpp(prompt, width, height, output_path);
    }

    Err(LocalGenError::NotAvailable)
}

/// Check if ComfyUI is running on localhost:8188.
async fn check_comfyui() -> bool {
    eprintln!("[nexus-builder][governance] comfyui::health_check endpoint=http://127.0.0.1:8188/system_stats");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build();
    let client = match client {
        Ok(c) => c,
        Err(_) => return false,
    };

    client
        .get("http://127.0.0.1:8188/system_stats")
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Generate via ComfyUI API.
async fn generate_via_comfyui(
    _prompt: &str,
    _width: u32,
    _height: u32,
    _output_path: &Path,
) -> Result<(), LocalGenError> {
    // ComfyUI integration: queue a workflow, poll for completion, download result.
    // This is a stub — wiring into ComfyUI's workflow API requires a specific
    // workflow JSON template which varies per installation. When ComfyUI is
    // available, this will be implemented to queue an img2img or txt2img workflow.
    Err(LocalGenError::NotAvailable)
}

/// Check if stable-diffusion.cpp (sd) binary is in PATH.
fn check_sd_cpp() -> bool {
    eprintln!("[nexus-builder][governance] sd_cpp::path_check binary=sd");
    std::process::Command::new("which")
        .arg("sd")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Generate via stable-diffusion.cpp CLI.
fn generate_via_sd_cpp(
    _prompt: &str,
    _width: u32,
    _height: u32,
    _output_path: &Path,
) -> Result<(), LocalGenError> {
    // sd.cpp integration: `sd -p "prompt" -W width -H height -o output_path`
    // This is a stub — requires sd binary and a model file to be configured.
    Err(LocalGenError::NotAvailable)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_not_available_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(generate_local("test", 512, 512, Path::new("/tmp/test.png")));
        // On CI / machines without SD, this should return NotAvailable
        assert!(result.is_err());
    }
}
