//! Safe wrapper around llama.cpp model loading and metadata extraction.

use std::ffi::{CStr, CString};
use std::io::Read as _;
use std::ptr;

use tracing::info;

use crate::error::LlamaError;
use crate::ffi;
use crate::types::{MemoryUsage, ModelLoadConfig, ModelMetadata};

/// A loaded GGUF model. Owns the underlying `llama_model` pointer and frees
/// it on drop.
pub struct LlamaModel {
    ptr: *mut ffi::LlamaModel,
    metadata: ModelMetadata,
}

// llama_model is thread-safe for concurrent read access (multiple contexts
// can share one model). Mutation only occurs through llama_model_free.
unsafe impl Send for LlamaModel {}
unsafe impl Sync for LlamaModel {}

impl LlamaModel {
    /// Load a GGUF model from disk.
    pub fn load(config: &ModelLoadConfig) -> Result<Self, LlamaError> {
        let c_path =
            CString::new(config.model_path.as_str()).map_err(|_| LlamaError::ModelLoadFailed {
                path: config.model_path.clone(),
                reason: "path contains null byte".into(),
            })?;

        let params = unsafe { ffi::nexus_model_params_create() };
        if params.is_null() {
            return Err(LlamaError::ModelLoadFailed {
                path: config.model_path.clone(),
                reason: "failed to allocate model params".into(),
            });
        }
        unsafe {
            ffi::nexus_model_params_set_n_gpu_layers(params, config.n_gpu_layers);
            ffi::nexus_model_params_set_use_mmap(params, config.use_mmap);
            ffi::nexus_model_params_set_use_mlock(params, config.use_mlock);
        }

        let ptr = unsafe { ffi::nexus_model_load_from_file(c_path.as_ptr(), params) };
        unsafe { ffi::nexus_model_params_free(params) };
        if ptr.is_null() {
            return Err(LlamaError::ModelLoadFailed {
                path: config.model_path.clone(),
                reason: "llama_model_load_from_file returned null".into(),
            });
        }

        let metadata = Self::extract_metadata(ptr);

        info!(
            arch = %metadata.architecture,
            params = metadata.total_params,
            ctx = metadata.context_length,
            moe = metadata.is_moe,
            "model loaded"
        );

        // CPU pinning + VM tuning require root and are handled by
        // `scripts/moe-turbo.sh`. We still attempt pinning here as a
        // best-effort fallback for users who run Tauri as root (rare).
        #[cfg(target_os = "linux")]
        if metadata.is_moe {
            pin_to_physical_cores();
        }

        // Pre-warm the OS page cache in a background thread.
        // mmap uses demand paging — expert weights only enter RAM on first access,
        // which causes random 4KB page faults that underutilize NVMe bandwidth.
        // Reading the file sequentially forces the kernel to pull data in large
        // contiguous chunks, filling the page cache so subsequent mmap accesses
        // hit RAM instead of SSD.
        let warmup_path = config.model_path.clone();
        let file_size = metadata.file_size_bytes;
        std::thread::spawn(move || {
            #[cfg(target_os = "linux")]
            {
                // NVMe readahead: try blockdev (needs root), fall back to
                // per-file posix_fadvise SEQUENTIAL in madvise_warmup().
                let _ = std::process::Command::new("blockdev")
                    .args(["--setra", "131072", "/dev/nvme0n1"])
                    .output();
            }

            // Sequentially read all GGUF files in the model directory to fill
            // the page cache. Multi-split models (e.g. 397B) produce several
            // .gguf files — warm them all, not just the primary one.
            let primary = std::path::Path::new(&warmup_path);
            let dir = primary.parent();
            let size_gb = file_size / (1024 * 1024 * 1024);
            eprintln!(
                "[flash] Pre-warming page cache for {} GB model in background",
                size_gb
            );

            // Use 16 MB read buffer — larger buffers reduce syscall overhead
            // and align better with NVMe's 128 KB internal page size.
            let mut buf = vec![0u8; 16 * 1024 * 1024];

            // Collect all .gguf files in the same directory
            let mut gguf_files: Vec<std::path::PathBuf> = Vec::new();
            if let Some(dir) = dir {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        if name.to_string_lossy().ends_with(".gguf") {
                            gguf_files.push(entry.path());
                        }
                    }
                }
            }
            // If no siblings found, just warm the primary file
            if gguf_files.is_empty() {
                gguf_files.push(primary.to_path_buf());
            }

            // Sort files by name to warm them in the same order llama.cpp reads.
            // Smallest shard first (usually metadata/vocab) so it's fully cached
            // before larger expert shards start streaming.
            gguf_files.sort();
            gguf_files.sort_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(u64::MAX));

            for path in &gguf_files {
                // Use madvise(MADV_SEQUENTIAL) via mmap for optimal kernel prefetch,
                // then madvise(MADV_WILLNEED) to force pages into cache.
                // This is ~2x faster than read() for warmup because the kernel can
                // issue async readahead without copying data to userspace.
                #[cfg(target_os = "linux")]
                {
                    if let Ok(file_len) = std::fs::metadata(path).map(|m| m.len()) {
                        if madvise_warmup(path, file_len) {
                            continue; // madvise warmup succeeded, skip read() fallback
                        }
                    }
                }

                // Fallback: sequential read for non-Linux or if madvise failed
                if let Ok(mut f) = std::fs::File::open(path) {
                    loop {
                        match f.read(&mut buf) {
                            Ok(0) => break,
                            Ok(_) => continue,
                            Err(_) => break,
                        }
                    }
                }
            }
            eprintln!(
                "[flash] Page cache warmup complete ({} files)",
                gguf_files.len()
            );
        });

        Ok(Self { ptr, metadata })
    }

    /// Cached model metadata.
    pub fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    /// Estimate memory usage for a given context size.
    pub fn estimate_memory(&self, ctx_size: u32) -> MemoryUsage {
        let model_size_mb = self.metadata.file_size_bytes / (1024 * 1024);
        // Rough estimate: 2 bytes per token per layer for KV cache
        let kv_bytes = ctx_size as u64
            * self.metadata.num_layers as u64
            * self.metadata.embedding_size as u64
            * 2;
        let context_size_mb = kv_bytes / (1024 * 1024);
        MemoryUsage {
            model_size_mb,
            context_size_mb,
            total_mb: model_size_mb + context_size_mb,
        }
    }

    /// Raw pointer for use by [`LlamaContext`](crate::context::LlamaContext).
    pub(crate) fn as_mut_ptr(&self) -> *mut ffi::LlamaModel {
        self.ptr
    }

    /// Get the vocab pointer from this model.
    pub(crate) fn vocab(&self) -> *const ffi::LlamaVocab {
        unsafe { ffi::llama_model_get_vocab(self.ptr) }
    }

    fn extract_metadata(ptr: *mut ffi::LlamaModel) -> ModelMetadata {
        let total_params = unsafe { ffi::llama_model_n_params(ptr) };
        let file_size_bytes = unsafe { ffi::llama_model_size(ptr) };
        let context_length = unsafe { ffi::llama_model_n_ctx_train(ptr) } as u32;

        let vocab_ptr = unsafe { ffi::llama_model_get_vocab(ptr) };
        let vocab_size = if vocab_ptr.is_null() {
            0u32
        } else {
            (unsafe { ffi::llama_vocab_n_tokens(vocab_ptr) }) as u32
        };

        let architecture = Self::read_meta_str(ptr, "general.architecture");
        let quantization = Self::read_meta_str(ptr, "general.quantization_version");

        let expert_count_str = Self::read_meta_str(ptr, "llama.expert_count");
        let expert_used_str = Self::read_meta_str(ptr, "llama.expert_used_count");
        let num_experts = expert_count_str.parse::<u32>().ok();
        let num_active_experts = expert_used_str.parse::<u32>().ok();
        let is_moe = num_experts.is_some_and(|n| n > 1);

        let num_layers_str = Self::read_meta_str(ptr, "llama.block_count");
        let num_layers = num_layers_str.parse::<u32>().unwrap_or(0);

        let embd_str = Self::read_meta_str(ptr, "llama.embedding_length");
        let embedding_size = embd_str.parse::<u32>().unwrap_or(0);

        ModelMetadata {
            architecture,
            total_params,
            file_size_bytes,
            context_length,
            vocab_size,
            quantization,
            is_moe,
            num_experts,
            num_active_experts,
            num_layers,
            embedding_size,
        }
    }

    fn read_meta_str(model: *mut ffi::LlamaModel, key: &str) -> String {
        let c_key = match CString::new(key) {
            Ok(k) => k,
            Err(_) => return String::new(),
        };
        let mut buf = [0i8; 256];
        let ret = unsafe {
            ffi::llama_model_meta_val_str(
                model,
                c_key.as_ptr(),
                buf.as_mut_ptr() as *mut libc::c_char,
                buf.len(),
            )
        };
        if ret < 0 {
            return String::new();
        }
        let c_str = unsafe { CStr::from_ptr(buf.as_ptr() as *const libc::c_char) };
        c_str.to_string_lossy().into_owned()
    }
}

/// Warm the page cache for a model file using mmap + madvise.
///
/// This is faster than read() because:
/// 1. `MADV_SEQUENTIAL` tells the kernel to readahead aggressively
/// 2. `MADV_WILLNEED` forces asynchronous page-in without copying to userspace
/// 3. The kernel batches I/O internally, saturating NVMe bandwidth (~3.5 GB/s)
///
/// After warmup, we switch to `MADV_RANDOM` so the kernel doesn't evict
/// recently-used expert pages when the MoE router accesses them non-sequentially.
#[cfg(target_os = "linux")]
fn madvise_warmup(path: &std::path::Path, file_len: u64) -> bool {
    use std::os::unix::io::AsRawFd;

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let fd = file.as_raw_fd();
    let len = file_len as libc::size_t;
    if len == 0 {
        return false;
    }

    // posix_fadvise SEQUENTIAL: per-fd readahead hint (doesn't need root).
    // This is the non-root equivalent of `blockdev --setra` and tells the
    // kernel to double its readahead window for this file descriptor.
    unsafe {
        libc::posix_fadvise(fd, 0, file_len as libc::off_t, libc::POSIX_FADV_SEQUENTIAL);
    }

    // SAFETY: mmap with MAP_PRIVATE|PROT_READ is safe — we never write.
    let ptr = unsafe {
        libc::mmap(
            ptr::null_mut(),
            len,
            libc::PROT_READ,
            libc::MAP_PRIVATE,
            fd,
            0,
        )
    };

    if ptr == libc::MAP_FAILED {
        return false;
    }

    // Request transparent huge pages (2 MB) for this mapping.
    // THP is in "madvise" mode on most systems — this opts in.
    // Reduces TLB misses by 512x (4 KB → 2 MB pages), critical for
    // MoE expert weight streaming which does millions of random lookups.
    // MADV_HUGEPAGE = 14 on Linux x86_64.
    const MADV_HUGEPAGE: libc::c_int = 14;
    unsafe { libc::madvise(ptr, len, MADV_HUGEPAGE) };

    // Tell kernel we'll read sequentially first (maximizes readahead)
    unsafe { libc::madvise(ptr, len, libc::MADV_SEQUENTIAL) };

    // Force pages into cache. Try MADV_POPULATE_READ first (Linux 5.14+):
    // it faults pages synchronously and is faster than MADV_WILLNEED because
    // the kernel doesn't need to schedule async I/O.
    // Fall back to MADV_WILLNEED in chunks if POPULATE_READ isn't available.
    const MADV_POPULATE_READ: libc::c_int = 22; // Linux 5.14+
    let populate_ok = unsafe { libc::madvise(ptr, len, MADV_POPULATE_READ) } == 0;

    if !populate_ok {
        // Fallback: MADV_WILLNEED in 256 MB chunks
        let chunk_size: usize = 256 * 1024 * 1024;
        let mut offset: usize = 0;
        while offset < len {
            let remaining = len - offset;
            let this_chunk = remaining.min(chunk_size);
            unsafe {
                libc::madvise(
                    (ptr as *mut u8).add(offset) as *mut libc::c_void,
                    this_chunk,
                    libc::MADV_WILLNEED,
                );
            }
            offset += this_chunk;
        }
    }

    // Now switch to RANDOM so the kernel doesn't evict recently-used expert
    // pages during non-sequential MoE inference access patterns.
    unsafe { libc::madvise(ptr, len, libc::MADV_RANDOM) };

    unsafe { libc::munmap(ptr, len) };

    true
}

/// Pin the current process to physical CPU cores (even-numbered on HT systems).
///
/// MoE inference is memory-bandwidth-bound. When the scheduler migrates threads
/// between cores, L1/L2 caches are cold and expert weight pages must be re-fetched.
/// Pinning to physical cores (0,2,4,6,8,10,12,14 on a 16-logical-core system)
/// eliminates this overhead.
#[cfg(target_os = "linux")]
fn pin_to_physical_cores() {
    // Read physical core IDs from /sys — only use core_id 0..N on each package.
    // On HT systems, logical cores 0,2,4,6... map to physical cores and
    // 1,3,5,7... are the sibling hyperthreads.
    let mut physical_cpus: Vec<usize> = Vec::new();
    let n_cpus = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) } as usize;

    for cpu in 0..n_cpus {
        let sibling_path = format!(
            "/sys/devices/system/cpu/cpu{}/topology/thread_siblings_list",
            cpu
        );
        if let Ok(content) = std::fs::read_to_string(&sibling_path) {
            // thread_siblings_list is like "0,8" or "0-1" — take the first number
            let first = content
                .trim()
                .split([',', '-'])
                .next()
                .and_then(|s| s.parse::<usize>().ok());
            if let Some(first_cpu) = first {
                // Only include this CPU if it IS the first in its sibling group.
                // Cap at 6 cores — MoE inference is memory-bound and 6 threads
                // has lower L3/TLB contention than 8 on consumer DDR4 systems.
                if first_cpu == cpu && physical_cpus.len() < 6 {
                    physical_cpus.push(cpu);
                }
            }
        }
    }

    if physical_cpus.is_empty() {
        return;
    }

    // Build a CPU set with only physical cores.
    // Set affinity for ALL threads in this process by iterating /proc/self/task.
    unsafe {
        let mut cpuset: libc::cpu_set_t = std::mem::zeroed();
        for &cpu in &physical_cpus {
            libc::CPU_SET(cpu, &mut cpuset);
        }

        // First, set for the main process (affects new threads)
        let pid = libc::getpid();
        let ret = libc::sched_setaffinity(pid, std::mem::size_of::<libc::cpu_set_t>(), &cpuset);

        // Also set for all existing threads
        let mut pinned_threads = 0u32;
        if let Ok(entries) = std::fs::read_dir("/proc/self/task") {
            for entry in entries.flatten() {
                if let Ok(tid) = entry.file_name().to_string_lossy().parse::<i32>() {
                    if libc::sched_setaffinity(tid, std::mem::size_of::<libc::cpu_set_t>(), &cpuset)
                        == 0
                    {
                        pinned_threads += 1;
                    }
                }
            }
        }

        if ret == 0 {
            eprintln!(
                "[flash] Pinned {} threads to {} physical cores: {:?}",
                pinned_threads,
                physical_cpus.len(),
                physical_cpus
            );
        }
    }
}

impl Drop for LlamaModel {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { ffi::llama_model_free(self.ptr) };
            self.ptr = ptr::null_mut();
        }
    }
}
