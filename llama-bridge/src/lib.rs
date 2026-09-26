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
pub mod chat_template;
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
        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
        {
            let mut system = sysinfo::System::new();
            system.refresh_memory_specifics(sysinfo::MemoryRefreshKind::new().with_ram());
            let total_bytes = system.total_memory();
            total_ram_mib_from_bytes(total_bytes).unwrap_or_else(|| {
                tracing::warn!(
                    total_bytes,
                    "Total system RAM detection returned less than 1 MiB; reporting 0 (unavailable)"
                );
                0
            })
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
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

#[cfg(any(test, target_os = "linux", target_os = "windows", target_os = "macos"))]
fn total_ram_mib_from_bytes(total_bytes: u64) -> Option<u64> {
    let total_mib = total_bytes / 1_048_576;
    (total_mib > 0).then_some(total_mib)
}

/// Hint the OS to start reading a file into the page cache.
///
/// Uses `posix_fadvise(POSIX_FADV_WILLNEED)` on Linux — a non-blocking
/// hint that tells the kernel to start asynchronous readahead. This is
/// the safe equivalent of `madvise(MADV_WILLNEED)` for file descriptors.
///
/// No-op on non-Linux platforms.
pub fn fadvise_willneed(file: &std::fs::File, len: usize) {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();
        // SAFETY: posix_fadvise is a standard POSIX syscall that only provides
        // an advisory hint. It cannot cause UB regardless of arguments.
        unsafe {
            libc::posix_fadvise(fd, 0, len as libc::off_t, libc::POSIX_FADV_WILLNEED);
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (file, len);
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

#[cfg(test)]
mod tests {
    use super::total_ram_mib_from_bytes;

    #[test]
    fn total_ram_zero_bytes_is_unavailable() {
        assert_eq!(total_ram_mib_from_bytes(0), None);
    }

    #[test]
    fn total_ram_sub_mib_is_unavailable() {
        assert_eq!(total_ram_mib_from_bytes(1), None);
        assert_eq!(total_ram_mib_from_bytes(1_048_575), None);
    }

    #[test]
    fn total_ram_exactly_one_mib() {
        assert_eq!(total_ram_mib_from_bytes(1_048_576), Some(1));
    }

    #[test]
    fn total_ram_sixteen_gib() {
        assert_eq!(total_ram_mib_from_bytes(17_179_869_184), Some(16_384));
    }

    #[test]
    fn total_ram_fractional_mib_truncates_downward() {
        assert_eq!(total_ram_mib_from_bytes(1_572_864), Some(1));
    }

    #[test]
    fn total_ram_above_four_gib() {
        assert_eq!(total_ram_mib_from_bytes(5_368_709_120), Some(5_120));
    }

    #[test]
    fn total_ram_max_u64_preserves_large_mib_value() {
        assert_eq!(total_ram_mib_from_bytes(u64::MAX), Some(17_592_186_044_415));
    }
}
