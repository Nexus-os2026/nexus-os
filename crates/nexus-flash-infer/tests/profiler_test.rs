use nexus_flash_infer::budget::MemoryBudget;
use nexus_flash_infer::profiler::ModelProfile;
use nexus_flash_infer::types::{HardwareInfo, RamType, SsdType};
use nexus_llama_bridge::ModelMetadata;

fn test_hw() -> HardwareInfo {
    HardwareInfo {
        total_ram_mb: 32768,
        cpu_cores: 16,
        has_avx2: true,
        has_avx512: false,
        has_metal: false,
        has_cuda: false,
        ssd_type: SsdType::NVMe,
        ssd_read_speed_mb_s: 3500,
        numa_nodes: 1,
        ram_type: RamType::DDR5,
        mem_bandwidth_gbps: 11.0,
    }
}

fn test_qwen397b_profile() -> ModelProfile {
    ModelProfile {
        name: "Qwen3.5-397B-A17B".into(),
        architecture: "qwen3.5".into(),
        total_params: 397_000_000_000,
        file_size_mb: 209_000,
        quantization: "Q4_K_M".into(),
        is_moe: true,
        num_experts: 512,
        num_active_experts: 10,
        num_layers: 60,
        num_kv_heads: 32,
        head_dim: 128,
        dense_weight_size_mb: 5500,
        expert_weight_size_mb: 203_500,
        single_expert_mb: 6.625,
        total_experts: 30720,
        active_params: 17_000_000_000,
        flops_per_token: 34_000_000_000,
    }
}

#[test]
fn test_model_profile_qwen397b() {
    let profile = test_qwen397b_profile();
    assert!(profile.is_moe);
    assert_eq!(profile.num_experts, 512);
    assert_eq!(profile.num_layers, 60);
    assert!(
        profile.dense_weight_size_mb < 6000,
        "Dense should be <6000MB, got {}",
        profile.dense_weight_size_mb
    );
    assert!(
        profile.expert_weight_size_mb > 200000,
        "Experts should be >200000MB, got {}",
        profile.expert_weight_size_mb
    );
}

#[test]
fn test_performance_estimate() {
    let hw = test_hw();
    let profile = test_qwen397b_profile();
    let budget = MemoryBudget::calculate(&hw, &profile, 4096);
    let est = profile.estimate_performance(&hw, &budget);

    assert!(est.memory_feasible);
    assert!(
        est.estimated_tok_per_sec > 0.1,
        "tok/s should be > 0.1, got {}",
        est.estimated_tok_per_sec
    );
    assert!(
        est.estimated_tok_per_sec < 100.0,
        "tok/s should be realistic (<100), got {}",
        est.estimated_tok_per_sec
    );
}

#[test]
fn test_performance_infeasible() {
    let hw = HardwareInfo {
        total_ram_mb: 2048,
        cpu_cores: 2,
        ..test_hw()
    };
    let profile = test_qwen397b_profile();
    let budget = MemoryBudget::calculate(&hw, &profile, 4096);
    let est = profile.estimate_performance(&hw, &budget);

    assert!(!est.memory_feasible);
    assert_eq!(est.estimated_tok_per_sec, 0.0);
}

#[test]
fn test_profile_from_metadata_moe() {
    let meta = ModelMetadata {
        architecture: "qwen3".into(),
        total_params: 30_000_000_000,
        file_size_bytes: 17_000_000_000,
        context_length: 32768,
        vocab_size: 150000,
        quantization: "Q4_K_M".into(),
        is_moe: true,
        num_experts: Some(128),
        num_active_experts: Some(4),
        num_layers: 48,
        embedding_size: 4096,
    };

    let profile = ModelProfile::from_metadata(&meta);
    assert!(profile.is_moe);
    assert_eq!(profile.num_experts, 128);
    assert_eq!(profile.num_active_experts, 4);
    assert_eq!(profile.num_layers, 48);
    assert!(profile.active_params < profile.total_params);
    assert!(profile.dense_weight_size_mb < profile.file_size_mb);
}

#[test]
fn test_profile_from_metadata_dense() {
    let meta = ModelMetadata {
        architecture: "llama".into(),
        total_params: 7_000_000_000,
        file_size_bytes: 4_400_000_000,
        context_length: 4096,
        vocab_size: 32000,
        quantization: "Q4_K_M".into(),
        is_moe: false,
        num_experts: None,
        num_active_experts: None,
        num_layers: 32,
        embedding_size: 4096,
    };

    let profile = ModelProfile::from_metadata(&meta);
    assert!(!profile.is_moe);
    assert_eq!(profile.num_experts, 0);
    assert_eq!(profile.active_params, profile.total_params);
    assert_eq!(profile.expert_weight_size_mb, 0);
}

#[test]
fn test_estimate_prompt_faster_than_generation() {
    let hw = test_hw();
    let profile = test_qwen397b_profile();
    let budget = MemoryBudget::calculate(&hw, &profile, 4096);
    let est = profile.estimate_performance(&hw, &budget);

    assert!(
        est.estimated_prompt_tok_per_sec >= est.estimated_tok_per_sec,
        "Prompt processing should be >= generation speed"
    );
}
