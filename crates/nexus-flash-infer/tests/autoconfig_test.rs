use nexus_flash_infer::autoconfig::auto_configure;
use nexus_flash_infer::profiler::ModelProfile;
use nexus_flash_infer::types::{
    HardwareInfo, InferencePreference, InferencePriority, RamType, SsdType,
};

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
fn test_autoconfig_cpu_only() {
    let hw = test_hw();
    let profile = test_qwen397b_profile();
    let pref = InferencePreference {
        model_path: "/models/qwen3.5-397b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    assert_eq!(config.load_config.n_gpu_layers, 0);
    assert!(config.load_config.use_mmap);
    assert!(config.load_config.cpu_moe);
}

#[test]
fn test_autoconfig_moe_batch_size() {
    let hw = test_hw();
    let profile = test_qwen397b_profile();
    let pref = InferencePreference {
        model_path: "/models/qwen3.5-397b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    // MoE models should use smaller batch sizes
    assert_eq!(config.generation_config.n_batch, 512);
}

#[test]
fn test_autoconfig_dense_batch_size() {
    let hw = test_hw();
    let profile = ModelProfile {
        name: "Dense-7B".into(),
        architecture: "llama".into(),
        total_params: 7_000_000_000,
        file_size_mb: 4400,
        quantization: "Q4_K_M".into(),
        is_moe: false,
        num_experts: 0,
        num_active_experts: 0,
        num_layers: 32,
        num_kv_heads: 32,
        head_dim: 128,
        dense_weight_size_mb: 4400,
        expert_weight_size_mb: 0,
        single_expert_mb: 0.0,
        total_experts: 0,
        active_params: 7_000_000_000,
        flops_per_token: 14_000_000_000,
    };
    let pref = InferencePreference {
        model_path: "/models/llama-7b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    // Dense models use larger batch sizes
    assert_eq!(config.generation_config.n_batch, 2048);
}

#[test]
fn test_autoconfig_suggests_smaller_model() {
    let hw = HardwareInfo {
        total_ram_mb: 4096,
        cpu_cores: 4,
        ..test_hw()
    };
    let profile = test_qwen397b_profile();
    let pref = InferencePreference {
        model_path: "/models/qwen3.5-397b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    // Large models should still load — mmap streams from disk.
    // auto_configure now warns instead of rejecting.
    let result = auto_configure(&hw, &profile, pref);
    assert!(
        result.is_ok(),
        "auto_configure should not reject large models"
    );
}

#[test]
fn test_autoconfig_numa_distribute() {
    let hw = HardwareInfo {
        numa_nodes: 2,
        ..test_hw()
    };
    let profile = test_qwen397b_profile();
    let pref = InferencePreference {
        model_path: "/models/qwen3.5-397b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    assert_eq!(
        config.load_config.numa_strategy,
        nexus_llama_bridge::NumaStrategy::Distribute
    );
}

#[test]
fn test_autoconfig_speed_priority_limits_context() {
    let hw = test_hw();
    let profile = test_qwen397b_profile();
    let pref = InferencePreference {
        model_path: "/models/qwen3.5-397b.gguf".into(),
        target_context_len: 32768,
        priority: InferencePriority::Speed,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    // Speed priority should cap context at 2048
    assert!(config.generation_config.n_ctx <= 2048);
}

#[test]
fn test_autoconfig_thread_cap() {
    let hw = HardwareInfo {
        cpu_cores: 64,
        ..test_hw()
    };
    let profile = test_qwen397b_profile();
    let pref = InferencePreference {
        model_path: "/models/qwen3.5-397b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    // Huge MoE (>200 GB) + DDR5: physical_cores.clamp(4, 6) = 6
    assert_eq!(config.load_config.n_threads, Some(6));
}

#[test]
fn test_autoconfig_gpu_layers_small_model() {
    let hw = HardwareInfo {
        has_cuda: true,
        ..test_hw()
    };
    let profile = ModelProfile {
        name: "Gemma-2B".into(),
        architecture: "gemma".into(),
        total_params: 2_000_000_000,
        file_size_mb: 1500,
        quantization: "Q4_K_M".into(),
        is_moe: false,
        num_experts: 0,
        num_active_experts: 0,
        num_layers: 18,
        num_kv_heads: 8,
        head_dim: 128,
        dense_weight_size_mb: 1500,
        expert_weight_size_mb: 0,
        single_expert_mb: 0.0,
        total_experts: 0,
        active_params: 2_000_000_000,
        flops_per_token: 4_000_000_000,
    };
    let pref = InferencePreference {
        model_path: "/models/gemma-2b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    // GPU offload disabled — force CPU-only for all models
    assert_eq!(config.load_config.n_gpu_layers, 0);
}

#[test]
fn test_autoconfig_gpu_layers_medium_model() {
    let hw = HardwareInfo {
        has_cuda: true,
        ..test_hw()
    };
    let profile = ModelProfile {
        name: "Dense-13B".into(),
        architecture: "llama".into(),
        total_params: 13_000_000_000,
        file_size_mb: 10_000, // ~9.8 GB — between 8 and 25 GB
        quantization: "Q4_K_M".into(),
        is_moe: false,
        num_experts: 0,
        num_active_experts: 0,
        num_layers: 40,
        num_kv_heads: 40,
        head_dim: 128,
        dense_weight_size_mb: 10_000,
        expert_weight_size_mb: 0,
        single_expert_mb: 0.0,
        total_experts: 0,
        active_params: 13_000_000_000,
        flops_per_token: 26_000_000_000,
    };
    let pref = InferencePreference {
        model_path: "/models/llama-13b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    // GPU offload disabled — force CPU-only for all models
    assert_eq!(config.load_config.n_gpu_layers, 0);
}

#[test]
fn test_autoconfig_gpu_layers_huge_moe_model() {
    let hw = HardwareInfo {
        has_cuda: true,
        ..test_hw()
    };
    let profile = test_qwen397b_profile();
    let pref = InferencePreference {
        model_path: "/models/qwen3.5-397b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    // Huge MoE (>50 GB) with CUDA: pure CPU — GPU PCIe overhead hurts
    // MoE throughput (measured: 0.1 tok/s with GPU vs 0.26 without).
    assert_eq!(config.load_config.n_gpu_layers, 0);
}

#[test]
fn test_autoconfig_gpu_layers_huge_dense_model() {
    let hw = HardwareInfo {
        has_cuda: true,
        ..test_hw()
    };
    let profile = ModelProfile {
        name: "Dense-70B".into(),
        architecture: "llama".into(),
        total_params: 70_000_000_000,
        file_size_mb: 40_000, // ~39 GB — huge dense model
        quantization: "Q4_K_M".into(),
        is_moe: false,
        num_experts: 0,
        num_active_experts: 0,
        num_layers: 80,
        num_kv_heads: 64,
        head_dim: 128,
        dense_weight_size_mb: 40_000,
        expert_weight_size_mb: 0,
        single_expert_mb: 0.0,
        total_experts: 0,
        active_params: 70_000_000_000,
        flops_per_token: 140_000_000_000,
    };
    let pref = InferencePreference {
        model_path: "/models/llama-70b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    // Huge dense model (>25 GB) — CPU only, GPU can't fit enough layers
    assert_eq!(config.load_config.n_gpu_layers, 0);
}

#[test]
fn test_autoconfig_no_gpu_without_cuda() {
    let hw = HardwareInfo {
        has_cuda: false,
        ..test_hw()
    };
    let profile = ModelProfile {
        name: "Gemma-2B".into(),
        architecture: "gemma".into(),
        total_params: 2_000_000_000,
        file_size_mb: 1500,
        quantization: "Q4_K_M".into(),
        is_moe: false,
        num_experts: 0,
        num_active_experts: 0,
        num_layers: 18,
        num_kv_heads: 8,
        head_dim: 128,
        dense_weight_size_mb: 1500,
        expert_weight_size_mb: 0,
        single_expert_mb: 0.0,
        total_experts: 0,
        active_params: 2_000_000_000,
        flops_per_token: 4_000_000_000,
    };
    let pref = InferencePreference {
        model_path: "/models/gemma-2b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    // No CUDA — CPU only
    assert_eq!(config.load_config.n_gpu_layers, 0);
}

#[test]
fn test_autoconfig_mlock_huge_model() {
    let hw = test_hw();
    let profile = test_qwen397b_profile();
    let pref = InferencePreference {
        model_path: "/models/qwen3.5-397b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    // mlock disabled — mlocking 44GB+ swaps the rest of the system
    assert!(!config.load_config.use_mlock);
}

#[test]
fn test_autoconfig_no_mlock_small_model() {
    let hw = test_hw();
    let profile = ModelProfile {
        name: "Dense-7B".into(),
        architecture: "llama".into(),
        total_params: 7_000_000_000,
        file_size_mb: 4400,
        quantization: "Q4_K_M".into(),
        is_moe: false,
        num_experts: 0,
        num_active_experts: 0,
        num_layers: 32,
        num_kv_heads: 32,
        head_dim: 128,
        dense_weight_size_mb: 4400,
        expert_weight_size_mb: 0,
        single_expert_mb: 0.0,
        total_experts: 0,
        active_params: 7_000_000_000,
        flops_per_token: 14_000_000_000,
    };
    let pref = InferencePreference {
        model_path: "/models/llama-7b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    // Small models don't need mlock
    assert!(!config.load_config.use_mlock);
}

#[test]
fn test_autoconfig_moe_physical_cores_only() {
    let hw = HardwareInfo {
        cpu_cores: 16, // 16 logical = 8 physical
        ..test_hw()
    };
    let profile = test_qwen397b_profile();
    let pref = InferencePreference {
        model_path: "/models/qwen3.5-397b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    // Huge MoE (>200 GB) + DDR5: physical_cores(8).clamp(4, 6) = 6
    assert_eq!(config.load_config.n_threads, Some(6));
}

#[test]
fn test_autoconfig_moe_large_ddr5_threads() {
    // 130 GB model (50-200 GB range) + DDR5 = physical_cores.clamp(6, 8)
    let hw = HardwareInfo {
        cpu_cores: 16, // 16 logical = 8 physical
        ..test_hw()
    };
    let profile = ModelProfile {
        file_size_mb: 131_000, // 128 GB — real IQ3_XXS quant
        ..test_qwen397b_profile()
    };
    let pref = InferencePreference {
        model_path: "/models/qwen3.5-397b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    // Large MoE (50-200 GB) + DDR5: physical_cores(8).clamp(6, 8) = 8
    assert_eq!(config.load_config.n_threads, Some(8));
}

#[test]
fn test_autoconfig_moe_ddr4_fewer_threads() {
    // Same model but DDR4 — should use fewer threads
    let hw = HardwareInfo {
        cpu_cores: 16,
        ram_type: RamType::DDR4,
        mem_bandwidth_gbps: 6.0,
        ..test_hw()
    };
    let profile = test_qwen397b_profile(); // 209 GB > 200
    let pref = InferencePreference {
        model_path: "/models/qwen3.5-397b.gguf".into(),
        target_context_len: 4096,
        priority: InferencePriority::Balanced,
        generation_config: None,
    };

    let config = auto_configure(&hw, &profile, pref).unwrap();
    // Extreme MoE + DDR4 (slow RAM): only 4 threads
    assert_eq!(config.load_config.n_threads, Some(4));
}
