//! Safe Rust bindings to llama.cpp for local LLM inference.
//!
//! This crate provides the FFI bridge between Nexus OS and llama.cpp,
//! enabling CPU-only GGUF model inference with mmap disk streaming and
//! MoE expert offloading.
//!
//! # Quick start
//!
//! ```no_run
//! use nexus_llama_bridge::{LlamaModel, LlamaContext, ModelLoadConfig, GenerationConfig, ControlFlow, TokenEvent};
//!
//! nexus_llama_bridge::init();
//!
//! let model = LlamaModel::load(&ModelLoadConfig {
//!     model_path: "/path/to/model.gguf".into(),
//!     ..Default::default()
//! }).unwrap();
//!
//! let config = GenerationConfig::default();
//! let mut ctx = LlamaContext::new(&model, &config).unwrap();
//!
//! ctx.generate_sync("Hello", &config, |event| {
//!     if let TokenEvent::Token { text, .. } = event {
//!         print!("{text}");
//!     }
//!     ControlFlow::Continue
//! }).unwrap();
//!
//! nexus_llama_bridge::cleanup();
//! ```

pub mod batch;
pub mod context;
pub mod error;
pub mod ffi;
pub mod model;
pub mod sampling;
pub mod tokenizer;
pub mod types;

// Convenience re-exports
pub use context::LlamaContext;
pub use error::LlamaError;
pub use model::LlamaModel;
pub use types::*;

use std::sync::atomic::{AtomicBool, Ordering};

static BACKEND_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize the llama.cpp backend. Safe to call multiple times — only the
/// first invocation has any effect.
pub fn init() {
    if !BACKEND_INITIALIZED.swap(true, Ordering::SeqCst) {
        unsafe { ffi::llama_backend_init() };
        tracing::info!("llama.cpp backend initialized");
    }
}

/// Shut down the llama.cpp backend. Call once at program exit.
pub fn cleanup() {
    if BACKEND_INITIALIZED.swap(false, Ordering::SeqCst) {
        unsafe { ffi::llama_backend_free() };
    }
}

/// Detect hardware capabilities for inference planning.
pub fn detect_hardware() -> HardwareInfo {
    let total_ram_mb = {
        #[cfg(target_os = "linux")]
        {
            // Read from /proc/meminfo
            std::fs::read_to_string("/proc/meminfo")
                .ok()
                .and_then(|s| {
                    s.lines()
                        .find(|l| l.starts_with("MemTotal:"))
                        .and_then(|l| {
                            l.split_whitespace()
                                .nth(1)
                                .and_then(|v| v.parse::<u64>().ok())
                        })
                })
                .map(|kb| kb / 1024)
                .unwrap_or(0)
        }
        #[cfg(not(target_os = "linux"))]
        {
            0u64
        }
    };

    let cpu_cores = std::thread::available_parallelism()
        .map(|p| p.get() as u32)
        .unwrap_or(1);

    HardwareInfo {
        total_ram_mb,
        cpu_cores,
        has_avx2: cfg!(target_feature = "avx2"),
        has_avx512: cfg!(target_feature = "avx512f"),
        has_metal: cfg!(target_os = "macos"),
        has_cuda: false, // would need runtime detection
        ssd_detected: detect_ssd(),
    }
}

fn detect_ssd() -> bool {
    #[cfg(target_os = "linux")]
    {
        // Check if root device is non-rotational
        std::fs::read_to_string("/sys/block/sda/queue/rotational")
            .or_else(|_| std::fs::read_to_string("/sys/block/nvme0n1/queue/rotational"))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .map(|v| v == 0)
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}
