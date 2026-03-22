use nexus_flash_infer::autoconfig::auto_configure;
use nexus_flash_infer::profiler::ModelProfile;
use nexus_flash_infer::types::{HardwareInfo, InferencePreference, InferencePriority, SsdType};

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
    // Threads capped at 32
    assert_eq!(config.load_config.n_threads, Some(32));
}
