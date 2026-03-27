use nexus_flash_infer::catalog::ModelCatalog;
use nexus_flash_infer::types::{HardwareInfo, ModelSpecialization, RamType, SsdType};

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

#[test]
fn test_catalog_has_50_plus_entries() {
    let catalog = ModelCatalog::new();
    assert!(
        catalog.entries().len() >= 50,
        "Catalog should have 50+ entries, got {}",
        catalog.entries().len()
    );
}

#[test]
fn test_catalog_search_qwen() {
    let catalog = ModelCatalog::new();
    let results = catalog.search("qwen");
    assert!(!results.is_empty(), "Should find Qwen models");
    for entry in &results {
        assert!(
            entry.name.to_lowercase().contains("qwen")
                || entry.provider.to_lowercase().contains("qwen"),
            "Search result should match query"
        );
    }
}

#[test]
fn test_catalog_search_case_insensitive() {
    let catalog = ModelCatalog::new();
    let lower = catalog.search("llama");
    let upper = catalog.search("LLAMA");
    assert_eq!(lower.len(), upper.len());
}

#[test]
fn test_catalog_best_quant_fits_ram() {
    let catalog = ModelCatalog::new();
    let hw = HardwareInfo {
        total_ram_mb: 8192,
        ..test_hw()
    };

    for entry in catalog.entries() {
        if let Some(quant) = catalog.best_quant(entry, &hw) {
            assert!(
                quant.min_ram_gb <= 8.0 * 0.85,
                "Best quant for {} should fit in 8GB: {} needs {}GB",
                entry.name,
                quant.quant_type,
                quant.min_ram_gb
            );
        }
    }
}

#[test]
fn test_catalog_recommend_32gb() {
    let catalog = ModelCatalog::new();
    let hw = test_hw();
    let recommendations = catalog.recommend(&hw);

    assert!(
        !recommendations.is_empty(),
        "Should have recommendations for 32GB system"
    );

    // Recommendations should be sorted by fitness (descending)
    for i in 1..recommendations.len() {
        assert!(
            recommendations[i - 1].fitness_score >= recommendations[i].fitness_score,
            "Recommendations should be sorted by fitness"
        );
    }
}

#[test]
fn test_catalog_recommend_4gb() {
    let catalog = ModelCatalog::new();
    let hw = HardwareInfo {
        total_ram_mb: 4096,
        cpu_cores: 4,
        ..test_hw()
    };
    let recommendations = catalog.recommend(&hw);

    // Should only recommend small models
    for rec in &recommendations {
        assert!(
            rec.best_quant.min_ram_gb <= 4.0 * 0.85,
            "4GB system should only get small model recommendations"
        );
    }
}

#[test]
fn test_catalog_has_code_models() {
    let catalog = ModelCatalog::new();
    let code_models: Vec<_> = catalog
        .entries()
        .iter()
        .filter(|e| e.specialization == ModelSpecialization::Code)
        .collect();
    assert!(
        code_models.len() >= 3,
        "Should have at least 3 code models, got {}",
        code_models.len()
    );
}

#[test]
fn test_catalog_has_math_models() {
    let catalog = ModelCatalog::new();
    let math_models: Vec<_> = catalog
        .entries()
        .iter()
        .filter(|e| e.specialization == ModelSpecialization::Math)
        .collect();
    assert!(!math_models.is_empty(), "Should have math models");
}

#[test]
fn test_catalog_entries_have_valid_quants() {
    let catalog = ModelCatalog::new();
    for entry in catalog.entries() {
        assert!(
            !entry.available_quants.is_empty(),
            "Model {} should have at least one quant",
            entry.name
        );
        for quant in &entry.available_quants {
            assert!(quant.file_size_gb > 0.0, "File size should be > 0");
            assert!(quant.min_ram_gb > 0.0, "Min RAM should be > 0");
            assert!(
                quant.quality_rating > 0.0 && quant.quality_rating <= 1.0,
                "Quality rating should be 0-1"
            );
        }
    }
}

#[test]
fn test_catalog_moe_models_have_experts() {
    let catalog = ModelCatalog::new();
    for entry in catalog.entries() {
        if entry.is_moe {
            assert!(
                entry.num_experts.is_some() && entry.num_experts.unwrap() > 1,
                "MoE model {} should have >1 experts",
                entry.name
            );
            assert!(
                entry.active_params.is_some(),
                "MoE model {} should have active_params",
                entry.name
            );
        }
    }
}
