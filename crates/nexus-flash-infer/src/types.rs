use serde::{Deserialize, Serialize};

/// Extended hardware information for inference planning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub total_ram_mb: u64,
    pub cpu_cores: u32,
    pub has_avx2: bool,
    pub has_avx512: bool,
    pub has_metal: bool,
    pub has_cuda: bool,
    pub ssd_type: SsdType,
    pub ssd_read_speed_mb_s: u32,
    pub numa_nodes: u32,
    /// Memory type: DDR4, DDR5, LPDDR5, etc.
    pub ram_type: RamType,
    /// Estimated single-stream memory bandwidth in GB/s.
    pub mem_bandwidth_gbps: f64,
}

/// System RAM type — affects memory bandwidth and optimal thread count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RamType {
    DDR4,
    DDR5,
    LPDDR5,
    LPDDR4,
    Unknown,
}

impl Default for HardwareInfo {
    fn default() -> Self {
        Self {
            total_ram_mb: 16384,
            cpu_cores: 8,
            has_avx2: true,
            has_avx512: false,
            has_metal: false,
            has_cuda: false,
            ssd_type: SsdType::NVMe,
            ssd_read_speed_mb_s: 3500,
            numa_nodes: 1,
            ram_type: RamType::Unknown,
            mem_bandwidth_gbps: 7.0,
        }
    }
}

/// SSD storage type, affects expert streaming performance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SsdType {
    NVMe,
    SATA,
    HDD,
    Unknown,
}

/// Model specialization category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelSpecialization {
    General,
    Code,
    Math,
    Creative,
    Multilingual,
    Vision,
}

/// Priority preference for inference tuning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum InferencePriority {
    /// Maximize tokens/second (smaller context, more expert cache).
    Speed,
    /// Maximize context window (less expert cache).
    Context,
    /// Balanced trade-off.
    #[default]
    Balanced,
}

/// User's inference preference for auto-configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferencePreference {
    pub model_path: String,
    pub target_context_len: u32,
    pub priority: InferencePriority,
    pub generation_config: Option<nexus_llama_bridge::GenerationConfig>,
}

/// Real-time inference performance metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceMetrics {
    pub session_id: String,
    pub tokens_per_second: f64,
    pub prompt_tokens_per_second: f64,
    pub memory_used_mb: u64,
    pub memory_budget_mb: u64,
    pub memory_utilization: f64,
    pub expert_cache_hit_rate: f64,
    pub io_read_mb_per_sec: f64,
    pub cpu_utilization: f64,
    pub context_used: u32,
    pub context_max: u32,
    pub total_tokens_generated: u64,
    pub uptime_seconds: f64,
}

/// Status of an inference session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionStatus {
    Loading,
    Ready,
    Generating,
    Idle,
    Error(String),
    Unloaded,
}

/// Summary info about an active session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub model_path: String,
    pub model_name: String,
    pub memory_used_mb: u64,
    pub tokens_generated: u64,
    pub status: SessionStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
