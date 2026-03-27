//! Universal flash inference engine with memory-budgeted orchestration.
//!
//! `nexus-flash-infer` sits between the Nexus agent system and inference backends,
//! adding memory budget enforcement, universal model profiling, hardware auto-configuration,
//! multi-backend abstraction, and session management.

pub mod autoconfig;
pub mod backend;
pub mod benchmark;
pub mod budget;
pub mod catalog;
#[cfg(feature = "download")]
pub mod downloader;
pub mod error;
pub mod hardware;
pub mod llama_backend;
pub mod monitor;
pub mod profiler;
pub mod registry;
pub mod session;
pub mod speculative;
pub mod types;

// Re-export key types at crate root for convenience.
pub use autoconfig::{auto_configure, OptimalConfig};
pub use backend::{InferenceBackend, LoadConfig, ModelFormat, ModelHandle};
pub use benchmark::{
    generate_report, run_full_benchmark, run_single_benchmark, standard_prompts, BenchmarkPrompt,
    BenchmarkResult,
};
pub use budget::MemoryBudget;
pub use catalog::{CatalogEntry, ModelCatalog, ModelRecommendation, QuantProfile};
pub use error::FlashError;
pub use hardware::detect_hardware;
pub use llama_backend::LlamaBackend;
pub use monitor::{aggregate_metrics, metrics_from_stats, AggregateMetrics, MetricsInput};
// Re-export llama-bridge types so callers don't need a direct dep.
pub use nexus_llama_bridge::{GenerationConfig, NumaStrategy};
pub use profiler::{ModelProfile, PerformanceEstimate};
pub use registry::BackendRegistry;
pub use session::SessionManager;
pub use speculative::{SpeculativeConfig, SpeculativeEngine};
pub use types::{
    HardwareInfo, InferencePreference, InferencePriority, ModelSpecialization, RamType,
    SessionInfo, SessionStatus, SsdType,
};

#[cfg(feature = "download")]
pub use downloader::{DownloadProgress, DownloadStatus, LocalModel, ModelDownloader, ModelStorage};
