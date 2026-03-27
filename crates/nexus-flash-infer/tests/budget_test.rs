use nexus_flash_infer::budget::MemoryBudget;
use nexus_flash_infer::profiler::ModelProfile;
use nexus_flash_infer::types::{HardwareInfo, RamType, SsdType};

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

fn test_qwen35b_profile() -> ModelProfile {
    ModelProfile {
        name: "Qwen3.5-35B-A3B".into(),
        architecture: "qwen3.5".into(),
        total_params: 35_000_000_000,
        file_size_mb: 20_000,
        quantization: "Q4_K_M".into(),
        is_moe: true,
        num_experts: 256,
        num_active_experts: 4,
        num_layers: 32,
        num_kv_heads: 16,
        head_dim: 128,
        dense_weight_size_mb: 2000,
        expert_weight_size_mb: 18_000,
        single_expert_mb: 2.2,
        total_experts: 8192,
        active_params: 3_000_000_000,
        flops_per_token: 6_000_000_000,
    }
}

#[test]
fn test_budget_32gb_qwen397b() {
    let hw = HardwareInfo {
        total_ram_mb: 32768,
        ..test_hw()
    };
    let model = test_qwen397b_profile();
    let budget = MemoryBudget::calculate(&hw, &model, 4096);

    assert!(budget.is_feasible());
    assert!(
        budget.expert_cache_mb > 10000,
        "Expert cache should be >10GB, got {}MB",
        budget.expert_cache_mb
    );
    assert!(
        budget.model_dense_mb < 6000,
        "Dense weights should be <6GB, got {}MB",
        budget.model_dense_mb
    );
}

#[test]
fn test_budget_16gb_qwen397b() {
    let hw = HardwareInfo {
        total_ram_mb: 16384,
        cpu_cores: 8,
        ..test_hw()
    };
    let model = test_qwen397b_profile();
    let budget = MemoryBudget::calculate(&hw, &model, 4096);

    // 16GB is tight for 397B — limited expert cache
    assert!(
        budget.expert_cache_mb < 8000,
        "Expert cache should be <8GB on 16GB system, got {}MB",
        budget.expert_cache_mb
    );
}

#[test]
fn test_budget_16gb_qwen35b_feasible() {
    let hw = HardwareInfo {
        total_ram_mb: 16384,
        cpu_cores: 8,
        ..test_hw()
    };
    let model = test_qwen35b_profile();
    let budget = MemoryBudget::calculate(&hw, &model, 8192);

    assert!(budget.is_feasible());
    assert!(
        budget.expert_cache_mb > 5000,
        "Expert cache should be >5GB, got {}MB",
        budget.expert_cache_mb
    );
}

#[test]
fn test_budget_4gb_tiny_model() {
    let hw = HardwareInfo {
        total_ram_mb: 4096,
        cpu_cores: 4,
        ..test_hw()
    };
    let model = ModelProfile {
        name: "TinyLlama".into(),
        architecture: "llama".into(),
        total_params: 1_100_000_000,
        file_size_mb: 700,
        quantization: "Q4_K_M".into(),
        is_moe: false,
        num_experts: 0,
        num_active_experts: 0,
        num_layers: 22,
        num_kv_heads: 4,
        head_dim: 64,
        dense_weight_size_mb: 700,
        expert_weight_size_mb: 0,
        single_expert_mb: 0.0,
        total_experts: 0,
        active_params: 1_100_000_000,
        flops_per_token: 2_200_000_000,
    };
    let budget = MemoryBudget::calculate(&hw, &model, 4096);
    assert!(budget.is_feasible());
}

#[test]
fn test_budget_max_context_length() {
    let hw = test_hw();
    let model = test_qwen397b_profile();
    let budget = MemoryBudget::calculate(&hw, &model, 4096);
    let max_ctx = budget.max_context_length(&model);
    assert!(max_ctx > 0, "Max context should be >0, got {}", max_ctx);
}

#[test]
fn test_budget_expert_cache_ratio() {
    let hw = test_hw();
    let model = test_qwen397b_profile();
    let budget = MemoryBudget::calculate(&hw, &model, 4096);
    let ratio = budget.expert_cache_ratio(&model);
    assert!(
        ratio > 0.0 && ratio <= 1.0,
        "Ratio should be 0-1, got {}",
        ratio
    );
}

#[test]
fn test_budget_dense_model_ratio_is_one() {
    let hw = test_hw();
    let model = ModelProfile {
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
    let budget = MemoryBudget::calculate(&hw, &model, 4096);
    let ratio = budget.expert_cache_ratio(&model);
    assert_eq!(ratio, 1.0, "Dense model should have ratio 1.0");
}
