use serde::{Deserialize, Serialize};

/// Configuration for loading a GGUF model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelLoadConfig {
    /// Filesystem path to the GGUF file.
    pub model_path: String,
    /// GPU layers to offload. 0 = CPU only, -1 = all layers.
    pub n_gpu_layers: i32,
    /// Memory-map the model file for disk streaming (default: true).
    pub use_mmap: bool,
    /// Lock model pages in RAM (default: false).
    pub use_mlock: bool,
    /// Enable CPU-based MoE expert offloading (default: true).
    pub cpu_moe: bool,
    /// Thread count. None = auto-detect.
    pub n_threads: Option<u32>,
    /// NUMA scheduling strategy.
    pub numa_strategy: NumaStrategy,
}

impl Default for ModelLoadConfig {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            n_gpu_layers: 0,
            use_mmap: true,
            use_mlock: false,
            cpu_moe: true,
            n_threads: None,
            numa_strategy: NumaStrategy::Disabled,
        }
    }
}

/// NUMA memory placement strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NumaStrategy {
    Disabled,
    Distribute,
    Isolate,
    Mirror,
}

/// KV cache quantization type (maps to ggml_type values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KvCacheType {
    /// f16 (default) — ggml_type 1
    F16,
    /// Q8_0 — ggml_type 8
    Q8_0,
    /// Q4_0 — ggml_type 2
    Q4_0,
}

impl KvCacheType {
    /// Returns the ggml_type integer value.
    pub fn as_ggml_type(self) -> i32 {
        match self {
            Self::F16 => 1,
            Self::Q8_0 => 8,
            Self::Q4_0 => 2,
        }
    }
}

/// Configuration for text generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    /// Maximum tokens to generate.
    pub max_tokens: u32,
    /// Sampling temperature (0.0 = greedy).
    pub temperature: f32,
    /// Nucleus sampling threshold.
    pub top_p: f32,
    /// Top-k sampling.
    pub top_k: i32,
    /// Min-p sampling threshold.
    pub min_p: f32,
    /// Repeat penalty.
    pub repeat_penalty: f32,
    /// Presence penalty.
    pub presence_penalty: f32,
    /// Frequency penalty.
    pub frequency_penalty: f32,
    /// RNG seed. None = random.
    pub seed: Option<u64>,
    /// Stop generation when any of these sequences appear.
    pub stop_sequences: Vec<String>,
    /// Context window size in tokens.
    pub n_ctx: u32,
    /// Logical batch size for prompt processing.
    pub n_batch: u32,
    /// Physical batch size (n_ubatch). 0 = use n_batch.
    pub n_ubatch: u32,
    /// Enable flash attention.
    pub flash_attn: bool,
    /// Override thread count for generation. None = auto-detect.
    pub n_threads: Option<u32>,
    /// KV cache quantization type for K. None = default (f16).
    /// Use `KvCacheType::Q8_0` or `KvCacheType::Q4_0` to reduce memory.
    pub type_k: Option<KvCacheType>,
    /// KV cache quantization type for V. None = default (f16).
    pub type_v: Option<KvCacheType>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            max_tokens: 2048,
            temperature: 0.0,    // Greedy — fastest (skips all stochastic samplers)
            top_p: 1.0,          // Disabled — no filtering overhead
            top_k: 0,            // Disabled
            min_p: 0.0,          // Disabled
            repeat_penalty: 1.1, // Prevent repetition loops (last 64 tokens)
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
            seed: None,
            stop_sequences: Vec::new(),
            n_ctx: 2048,   // Smaller KV cache = faster attention
            n_batch: 2048, // Large batch for fast prompt processing
            n_ubatch: 512, // Physical sub-batch
            flash_attn: true,
            n_threads: None,
            type_k: Some(KvCacheType::Q8_0), // 2× KV memory savings
            type_v: Some(KvCacheType::Q8_0),
        }
    }
}

impl GenerationConfig {
    /// Speed-optimized config: greedy sampling, small context, quantized KV cache.
    pub fn fast() -> Self {
        Self {
            max_tokens: 4096,
            temperature: 0.0,
            top_p: 1.0,
            top_k: 0,
            min_p: 0.0,
            repeat_penalty: 1.1,
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
            seed: None,
            stop_sequences: Vec::new(),
            n_ctx: 2048,
            n_batch: 2048,
            n_ubatch: 512,
            flash_attn: true,
            n_threads: None,
            type_k: Some(KvCacheType::Q8_0),
            type_v: Some(KvCacheType::Q8_0),
        }
    }

    /// Balanced config: light sampling with good quality, quantized KV cache.
    pub fn balanced() -> Self {
        Self {
            max_tokens: 4096,
            temperature: 0.6,
            top_p: 0.9,
            top_k: 0, // Disabled — top_p is sufficient
            min_p: 0.05,
            repeat_penalty: 1.1,
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
            seed: None,
            stop_sequences: Vec::new(),
            n_ctx: 4096,
            n_batch: 2048,
            n_ubatch: 512,
            flash_attn: true,
            n_threads: None,
            type_k: Some(KvCacheType::Q8_0),
            type_v: Some(KvCacheType::Q8_0),
        }
    }
}

/// Metadata extracted from a loaded GGUF model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub architecture: String,
    pub total_params: u64,
    pub file_size_bytes: u64,
    pub context_length: u32,
    pub vocab_size: u32,
    pub quantization: String,
    pub is_moe: bool,
    pub num_experts: Option<u32>,
    pub num_active_experts: Option<u32>,
    pub num_layers: u32,
    pub embedding_size: u32,
}

/// Performance statistics from a generation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfStats {
    pub tokens_generated: u32,
    pub prompt_tokens: u32,
    pub prompt_eval_time_ms: f64,
    pub generation_time_ms: f64,
    pub tokens_per_second: f64,
    pub prompt_tokens_per_second: f64,
    pub memory_used_mb: u64,
}

/// Memory usage breakdown for a model + context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    pub model_size_mb: u64,
    pub context_size_mb: u64,
    pub total_mb: u64,
}

/// Hardware capability detection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub total_ram_mb: u64,
    pub cpu_cores: u32,
    pub has_avx2: bool,
    pub has_avx512: bool,
    pub has_metal: bool,
    pub has_cuda: bool,
    pub ssd_detected: bool,
}

/// A streaming token event emitted during generation.
#[derive(Debug, Clone)]
pub enum TokenEvent {
    /// A new token was generated.
    Token { text: String, token_id: i32 },
    /// Generation completed successfully.
    Done { stats: PerfStats },
    /// An error occurred during generation.
    Error { message: String },
}

/// Controls whether the generation loop should continue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlFlow {
    Continue,
    Stop,
}
