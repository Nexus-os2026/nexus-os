use serde::{Deserialize, Serialize};

use crate::types::{HardwareInfo, ModelSpecialization};

/// Built-in catalog of popular models with pre-computed profiles.
pub struct ModelCatalog {
    entries: Vec<CatalogEntry>,
}

/// A catalog entry for a known model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub name: String,
    pub provider: String,
    pub huggingface_id: String,
    pub license: String,
    pub total_params: u64,
    pub is_moe: bool,
    pub active_params: Option<u64>,
    pub num_experts: Option<u32>,
    pub num_layers: u32,
    pub available_quants: Vec<QuantProfile>,
    pub specialization: ModelSpecialization,
}

/// Quantization profile for a model variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantProfile {
    pub quant_type: String,
    pub file_size_gb: f64,
    pub min_ram_gb: f64,
    pub quality_rating: f64,
}

/// A model recommendation with fitness score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRecommendation {
    pub entry: CatalogEntry,
    pub best_quant: QuantProfile,
    pub fitness_score: f64,
    pub estimated_tok_per_sec: f64,
    pub reason: String,
}

impl ModelCatalog {
    /// Create the built-in catalog with 50+ model entries.
    pub fn new() -> Self {
        Self {
            entries: build_catalog(),
        }
    }

    /// Get recommended models for given hardware.
    pub fn recommend(&self, hw: &HardwareInfo) -> Vec<ModelRecommendation> {
        let available_gb = hw.total_ram_mb as f64 / 1024.0;
        let mut recommendations = Vec::new();

        for entry in &self.entries {
            if let Some(quant) = self.best_quant(entry, hw) {
                if quant.min_ram_gb <= available_gb * 0.85 {
                    let fitness = compute_fitness(entry, quant, hw);
                    let tok_s = estimate_catalog_tok_s(entry, quant, hw);
                    recommendations.push(ModelRecommendation {
                        entry: entry.clone(),
                        best_quant: quant.clone(),
                        fitness_score: fitness,
                        estimated_tok_per_sec: tok_s,
                        reason: format!(
                            "{} at {} — fits in {:.0}GB RAM",
                            entry.name, quant.quant_type, available_gb
                        ),
                    });
                }
            }
        }

        recommendations.sort_by(|a, b| {
            b.fitness_score
                .partial_cmp(&a.fitness_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        recommendations
    }

    /// Search catalog by name (case-insensitive substring match).
    pub fn search(&self, query: &str) -> Vec<&CatalogEntry> {
        let q = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.name.to_lowercase().contains(&q)
                    || e.provider.to_lowercase().contains(&q)
                    || e.huggingface_id.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Find the best quantization for hardware constraints.
    pub fn best_quant<'a>(
        &self,
        entry: &'a CatalogEntry,
        hw: &HardwareInfo,
    ) -> Option<&'a QuantProfile> {
        let available_gb = hw.total_ram_mb as f64 / 1024.0;

        // Pick highest quality quant that fits
        let mut best: Option<&QuantProfile> = None;
        for q in &entry.available_quants {
            if q.min_ram_gb <= available_gb * 0.85 {
                match best {
                    None => best = Some(q),
                    Some(current) if q.quality_rating > current.quality_rating => {
                        best = Some(q);
                    }
                    _ => {}
                }
            }
        }
        best
    }

    /// Get all entries.
    pub fn entries(&self) -> &[CatalogEntry] {
        &self.entries
    }
}

impl Default for ModelCatalog {
    fn default() -> Self {
        Self::new()
    }
}

fn compute_fitness(entry: &CatalogEntry, quant: &QuantProfile, hw: &HardwareInfo) -> f64 {
    let quality = quant.quality_rating;
    let size_fit = 1.0 - (quant.min_ram_gb / (hw.total_ram_mb as f64 / 1024.0)).min(1.0);
    let param_score = if entry.is_moe {
        (entry.active_params.unwrap_or(entry.total_params) as f64).log10() / 12.0
    } else {
        (entry.total_params as f64).log10() / 12.0
    };
    quality * 0.4 + size_fit * 0.3 + param_score * 0.3
}

fn estimate_catalog_tok_s(entry: &CatalogEntry, quant: &QuantProfile, hw: &HardwareInfo) -> f64 {
    let active = entry.active_params.unwrap_or(entry.total_params) as f64;
    let bytes_per_param = match quant.quant_type.as_str() {
        t if t.contains("Q2") => 0.3,
        t if t.contains("Q3") => 0.4,
        t if t.contains("Q4") => 0.5,
        t if t.contains("Q5") => 0.65,
        t if t.contains("Q6") => 0.75,
        t if t.contains("Q8") => 1.0,
        _ => 0.5,
    };
    let model_bytes = active * bytes_per_param;
    // Rough memory bandwidth: 50 GB/s DDR4, scale with cores
    let bandwidth = 50e9 * (hw.cpu_cores as f64 / 8.0).min(2.0);
    if model_bytes > 0.0 {
        bandwidth / model_bytes
    } else {
        0.0
    }
}

/// Build the full catalog of 50+ models.
fn build_catalog() -> Vec<CatalogEntry> {
    vec![
        // === Qwen 3.5 Series (MoE) ===
        CatalogEntry {
            name: "Qwen3.5-397B-A17B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "unsloth/Qwen3.5-397B-A17B-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 397_000_000_000,
            is_moe: true,
            active_params: Some(17_000_000_000),
            num_experts: Some(512),
            num_layers: 60,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q2_K".into(),
                    file_size_gb: 130.0,
                    min_ram_gb: 18.0,
                    quality_rating: 0.82,
                },
                QuantProfile {
                    quant_type: "Q3_K_M".into(),
                    file_size_gb: 170.0,
                    min_ram_gb: 20.0,
                    quality_rating: 0.88,
                },
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 209.0,
                    min_ram_gb: 22.0,
                    quality_rating: 0.95,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Qwen3.5-195B-A14B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "unsloth/Qwen3.5-195B-A14B-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 195_000_000_000,
            is_moe: true,
            active_params: Some(14_000_000_000),
            num_experts: Some(256),
            num_layers: 48,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 105.0,
                    min_ram_gb: 16.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q6_K".into(),
                    file_size_gb: 140.0,
                    min_ram_gb: 20.0,
                    quality_rating: 0.97,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Qwen3.5-35B-A3B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "unsloth/Qwen3.5-35B-A3B-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 35_000_000_000,
            is_moe: true,
            active_params: Some(3_000_000_000),
            num_experts: Some(256),
            num_layers: 32,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 20.0,
                    min_ram_gb: 6.0,
                    quality_rating: 0.93,
                },
                QuantProfile {
                    quant_type: "Q6_K".into(),
                    file_size_gb: 26.0,
                    min_ram_gb: 8.0,
                    quality_rating: 0.96,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 34.0,
                    min_ram_gb: 10.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        // === Qwen 3 Series ===
        CatalogEntry {
            name: "Qwen3-235B-A22B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "Qwen/Qwen3-235B-A22B-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 235_000_000_000,
            is_moe: true,
            active_params: Some(22_000_000_000),
            num_experts: Some(128),
            num_layers: 94,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 130.0,
                    min_ram_gb: 24.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q2_K".into(),
                    file_size_gb: 78.0,
                    min_ram_gb: 16.0,
                    quality_rating: 0.83,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Qwen3-30B-A3B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "Qwen/Qwen3-30B-A3B-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 30_000_000_000,
            is_moe: true,
            active_params: Some(3_000_000_000),
            num_experts: Some(128),
            num_layers: 48,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 17.0,
                    min_ram_gb: 5.0,
                    quality_rating: 0.93,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 30.0,
                    min_ram_gb: 8.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Qwen3-32B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "Qwen/Qwen3-32B-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 32_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 64,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 19.0,
                    min_ram_gb: 10.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q6_K".into(),
                    file_size_gb: 25.0,
                    min_ram_gb: 14.0,
                    quality_rating: 0.97,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Qwen3-14B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "Qwen/Qwen3-14B-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 14_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 48,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 8.5,
                    min_ram_gb: 5.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 15.0,
                    min_ram_gb: 8.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Qwen3-8B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "Qwen/Qwen3-8B-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 8_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 36,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 5.0,
                    min_ram_gb: 3.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 8.5,
                    min_ram_gb: 5.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Qwen3-4B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "Qwen/Qwen3-4B-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 4_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 36,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 2.7,
                    min_ram_gb: 2.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 4.5,
                    min_ram_gb: 3.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Qwen3-1.7B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "Qwen/Qwen3-1.7B-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 1_700_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 28,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 1.2,
                    min_ram_gb: 1.5,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 2.0,
                    min_ram_gb: 2.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        // === Llama 4 Series ===
        CatalogEntry {
            name: "Llama-4-Maverick-17B-128E".into(),
            provider: "Meta".into(),
            huggingface_id: "meta-llama/Llama-4-Maverick-17B-128E-Instruct-GGUF".into(),
            license: "Llama-4".into(),
            total_params: 400_000_000_000,
            is_moe: true,
            active_params: Some(17_000_000_000),
            num_experts: Some(128),
            num_layers: 48,
            available_quants: vec![QuantProfile {
                quant_type: "Q4_K_M".into(),
                file_size_gb: 210.0,
                min_ram_gb: 24.0,
                quality_rating: 0.94,
            }],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Llama-4-Scout-17B-16E".into(),
            provider: "Meta".into(),
            huggingface_id: "meta-llama/Llama-4-Scout-17B-16E-Instruct-GGUF".into(),
            license: "Llama-4".into(),
            total_params: 109_000_000_000,
            is_moe: true,
            active_params: Some(17_000_000_000),
            num_experts: Some(16),
            num_layers: 48,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 60.0,
                    min_ram_gb: 16.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 110.0,
                    min_ram_gb: 24.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        // === Llama 3 Series ===
        CatalogEntry {
            name: "Llama-3.3-70B".into(),
            provider: "Meta".into(),
            huggingface_id: "meta-llama/Llama-3.3-70B-Instruct-GGUF".into(),
            license: "Llama-3.1".into(),
            total_params: 70_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 80,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 40.0,
                    min_ram_gb: 24.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q2_K".into(),
                    file_size_gb: 25.0,
                    min_ram_gb: 16.0,
                    quality_rating: 0.82,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Llama-3.1-8B".into(),
            provider: "Meta".into(),
            huggingface_id: "meta-llama/Llama-3.1-8B-Instruct-GGUF".into(),
            license: "Llama-3.1".into(),
            total_params: 8_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 32,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 4.9,
                    min_ram_gb: 3.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 8.5,
                    min_ram_gb: 5.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        // === DeepSeek Series ===
        CatalogEntry {
            name: "DeepSeek-R1".into(),
            provider: "DeepSeek".into(),
            huggingface_id: "deepseek-ai/DeepSeek-R1-GGUF".into(),
            license: "MIT".into(),
            total_params: 671_000_000_000,
            is_moe: true,
            active_params: Some(37_000_000_000),
            num_experts: Some(256),
            num_layers: 61,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q2_K".into(),
                    file_size_gb: 220.0,
                    min_ram_gb: 32.0,
                    quality_rating: 0.80,
                },
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 360.0,
                    min_ram_gb: 48.0,
                    quality_rating: 0.94,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "DeepSeek-R1-Distill-Qwen-32B".into(),
            provider: "DeepSeek".into(),
            huggingface_id: "deepseek-ai/DeepSeek-R1-Distill-Qwen-32B-GGUF".into(),
            license: "MIT".into(),
            total_params: 32_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 64,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 19.0,
                    min_ram_gb: 10.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 33.0,
                    min_ram_gb: 18.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::Math,
        },
        CatalogEntry {
            name: "DeepSeek-R1-Distill-Qwen-14B".into(),
            provider: "DeepSeek".into(),
            huggingface_id: "deepseek-ai/DeepSeek-R1-Distill-Qwen-14B-GGUF".into(),
            license: "MIT".into(),
            total_params: 14_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 48,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 8.5,
                    min_ram_gb: 5.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 15.0,
                    min_ram_gb: 8.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::Math,
        },
        CatalogEntry {
            name: "DeepSeek-R1-Distill-Llama-8B".into(),
            provider: "DeepSeek".into(),
            huggingface_id: "deepseek-ai/DeepSeek-R1-Distill-Llama-8B-GGUF".into(),
            license: "MIT".into(),
            total_params: 8_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 32,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 4.9,
                    min_ram_gb: 3.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 8.5,
                    min_ram_gb: 5.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::Math,
        },
        CatalogEntry {
            name: "DeepSeek-V3".into(),
            provider: "DeepSeek".into(),
            huggingface_id: "deepseek-ai/DeepSeek-V3-GGUF".into(),
            license: "DeepSeek".into(),
            total_params: 671_000_000_000,
            is_moe: true,
            active_params: Some(37_000_000_000),
            num_experts: Some(256),
            num_layers: 61,
            available_quants: vec![QuantProfile {
                quant_type: "Q4_K_M".into(),
                file_size_gb: 360.0,
                min_ram_gb: 48.0,
                quality_rating: 0.94,
            }],
            specialization: ModelSpecialization::Code,
        },
        // === Mistral / Mixtral ===
        CatalogEntry {
            name: "Mixtral-8x22B".into(),
            provider: "Mistral AI".into(),
            huggingface_id: "mistralai/Mixtral-8x22B-Instruct-v0.1-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 141_000_000_000,
            is_moe: true,
            active_params: Some(39_000_000_000),
            num_experts: Some(8),
            num_layers: 56,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 80.0,
                    min_ram_gb: 48.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q2_K".into(),
                    file_size_gb: 48.0,
                    min_ram_gb: 32.0,
                    quality_rating: 0.82,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Mixtral-8x7B".into(),
            provider: "Mistral AI".into(),
            huggingface_id: "TheBloke/Mixtral-8x7B-Instruct-v0.1-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 46_700_000_000,
            is_moe: true,
            active_params: Some(12_900_000_000),
            num_experts: Some(8),
            num_layers: 32,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 26.0,
                    min_ram_gb: 16.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 47.0,
                    min_ram_gb: 28.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Mistral-Small-24B".into(),
            provider: "Mistral AI".into(),
            huggingface_id: "mistralai/Mistral-Small-24B-Instruct-2501-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 24_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 40,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 14.0,
                    min_ram_gb: 8.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 25.0,
                    min_ram_gb: 14.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Mistral-7B".into(),
            provider: "Mistral AI".into(),
            huggingface_id: "TheBloke/Mistral-7B-Instruct-v0.2-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 7_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 32,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 4.4,
                    min_ram_gb: 3.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 7.7,
                    min_ram_gb: 5.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Codestral-25.01-24B".into(),
            provider: "Mistral AI".into(),
            huggingface_id: "mistralai/Codestral-25.01-24B-GGUF".into(),
            license: "MNPL".into(),
            total_params: 24_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 40,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 14.0,
                    min_ram_gb: 8.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 25.0,
                    min_ram_gb: 14.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::Code,
        },
        // === Gemma Series ===
        CatalogEntry {
            name: "Gemma-3-27B".into(),
            provider: "Google".into(),
            huggingface_id: "google/gemma-3-27b-it-GGUF".into(),
            license: "Gemma".into(),
            total_params: 27_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 46,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 16.0,
                    min_ram_gb: 10.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 28.0,
                    min_ram_gb: 16.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Gemma-3-12B".into(),
            provider: "Google".into(),
            huggingface_id: "google/gemma-3-12b-it-GGUF".into(),
            license: "Gemma".into(),
            total_params: 12_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 36,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 7.5,
                    min_ram_gb: 4.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 13.0,
                    min_ram_gb: 7.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Gemma-3-4B".into(),
            provider: "Google".into(),
            huggingface_id: "google/gemma-3-4b-it-GGUF".into(),
            license: "Gemma".into(),
            total_params: 4_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 26,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 2.7,
                    min_ram_gb: 2.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 4.5,
                    min_ram_gb: 3.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Gemma-3-1B".into(),
            provider: "Google".into(),
            huggingface_id: "google/gemma-3-1b-it-GGUF".into(),
            license: "Gemma".into(),
            total_params: 1_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 18,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 0.8,
                    min_ram_gb: 1.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 1.2,
                    min_ram_gb: 1.5,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        // === Phi Series ===
        CatalogEntry {
            name: "Phi-4-14B".into(),
            provider: "Microsoft".into(),
            huggingface_id: "microsoft/phi-4-GGUF".into(),
            license: "MIT".into(),
            total_params: 14_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 40,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 8.5,
                    min_ram_gb: 5.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 15.0,
                    min_ram_gb: 8.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Phi-4-Mini-3.8B".into(),
            provider: "Microsoft".into(),
            huggingface_id: "microsoft/Phi-4-mini-instruct-GGUF".into(),
            license: "MIT".into(),
            total_params: 3_800_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 32,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 2.5,
                    min_ram_gb: 2.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 4.2,
                    min_ram_gb: 3.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        // === Code Models ===
        CatalogEntry {
            name: "Qwen2.5-Coder-32B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "Qwen/Qwen2.5-Coder-32B-Instruct-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 32_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 64,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 19.0,
                    min_ram_gb: 10.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 33.0,
                    min_ram_gb: 18.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::Code,
        },
        CatalogEntry {
            name: "Qwen2.5-Coder-14B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "Qwen/Qwen2.5-Coder-14B-Instruct-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 14_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 48,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 8.5,
                    min_ram_gb: 5.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 15.0,
                    min_ram_gb: 8.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::Code,
        },
        CatalogEntry {
            name: "Qwen2.5-Coder-7B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 7_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 28,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 4.4,
                    min_ram_gb: 3.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 7.7,
                    min_ram_gb: 5.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::Code,
        },
        CatalogEntry {
            name: "CodeLlama-34B".into(),
            provider: "Meta".into(),
            huggingface_id: "TheBloke/CodeLlama-34B-Instruct-GGUF".into(),
            license: "Llama-2".into(),
            total_params: 34_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 48,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 20.0,
                    min_ram_gb: 12.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 35.0,
                    min_ram_gb: 20.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::Code,
        },
        CatalogEntry {
            name: "StarCoder2-15B".into(),
            provider: "BigCode".into(),
            huggingface_id: "bigcode/starcoder2-15b-GGUF".into(),
            license: "BigCode-OpenRAIL-M".into(),
            total_params: 15_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 40,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 9.0,
                    min_ram_gb: 5.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 16.0,
                    min_ram_gb: 9.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::Code,
        },
        // === Math / Reasoning ===
        CatalogEntry {
            name: "Qwen2.5-Math-72B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "Qwen/Qwen2.5-Math-72B-Instruct-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 72_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 80,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 42.0,
                    min_ram_gb: 24.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q2_K".into(),
                    file_size_gb: 26.0,
                    min_ram_gb: 16.0,
                    quality_rating: 0.82,
                },
            ],
            specialization: ModelSpecialization::Math,
        },
        CatalogEntry {
            name: "Qwen2.5-Math-7B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "Qwen/Qwen2.5-Math-7B-Instruct-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 7_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 28,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 4.4,
                    min_ram_gb: 3.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 7.7,
                    min_ram_gb: 5.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::Math,
        },
        // === Vision Models ===
        CatalogEntry {
            name: "LLaVA-v1.6-34B".into(),
            provider: "liuhaotian".into(),
            huggingface_id: "cjpais/llava-v1.6-34B-gguf".into(),
            license: "Apache-2.0".into(),
            total_params: 34_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 48,
            available_quants: vec![QuantProfile {
                quant_type: "Q4_K_M".into(),
                file_size_gb: 20.0,
                min_ram_gb: 12.0,
                quality_rating: 0.94,
            }],
            specialization: ModelSpecialization::Vision,
        },
        CatalogEntry {
            name: "Qwen2.5-VL-72B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "Qwen/Qwen2.5-VL-72B-Instruct-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 72_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 80,
            available_quants: vec![QuantProfile {
                quant_type: "Q4_K_M".into(),
                file_size_gb: 42.0,
                min_ram_gb: 24.0,
                quality_rating: 0.94,
            }],
            specialization: ModelSpecialization::Vision,
        },
        CatalogEntry {
            name: "Qwen2.5-VL-7B".into(),
            provider: "Alibaba/Qwen".into(),
            huggingface_id: "Qwen/Qwen2.5-VL-7B-Instruct-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 7_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 28,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 4.4,
                    min_ram_gb: 3.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 7.7,
                    min_ram_gb: 5.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::Vision,
        },
        // === Creative / Chat ===
        CatalogEntry {
            name: "Nous-Hermes-2-Mixtral-8x7B".into(),
            provider: "NousResearch".into(),
            huggingface_id: "TheBloke/Nous-Hermes-2-Mixtral-8x7B-DPO-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 46_700_000_000,
            is_moe: true,
            active_params: Some(12_900_000_000),
            num_experts: Some(8),
            num_layers: 32,
            available_quants: vec![QuantProfile {
                quant_type: "Q4_K_M".into(),
                file_size_gb: 26.0,
                min_ram_gb: 16.0,
                quality_rating: 0.94,
            }],
            specialization: ModelSpecialization::Creative,
        },
        CatalogEntry {
            name: "Yi-34B".into(),
            provider: "01-ai".into(),
            huggingface_id: "TheBloke/Yi-34B-Chat-GGUF".into(),
            license: "Yi License".into(),
            total_params: 34_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 60,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 20.0,
                    min_ram_gb: 12.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 35.0,
                    min_ram_gb: 20.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        // === Multilingual ===
        CatalogEntry {
            name: "Aya-Expanse-32B".into(),
            provider: "Cohere".into(),
            huggingface_id: "bartowski/aya-expanse-32b-GGUF".into(),
            license: "CC-BY-NC-4.0".into(),
            total_params: 32_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 64,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 19.0,
                    min_ram_gb: 10.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 33.0,
                    min_ram_gb: 18.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::Multilingual,
        },
        CatalogEntry {
            name: "Aya-Expanse-8B".into(),
            provider: "Cohere".into(),
            huggingface_id: "bartowski/aya-expanse-8b-GGUF".into(),
            license: "CC-BY-NC-4.0".into(),
            total_params: 8_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 32,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 5.0,
                    min_ram_gb: 3.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 8.5,
                    min_ram_gb: 5.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::Multilingual,
        },
        // === Small / Edge Models ===
        CatalogEntry {
            name: "TinyLlama-1.1B".into(),
            provider: "TinyLlama".into(),
            huggingface_id: "TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 1_100_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 22,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 0.7,
                    min_ram_gb: 1.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 1.2,
                    min_ram_gb: 1.5,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "SmolLM2-1.7B".into(),
            provider: "HuggingFace".into(),
            huggingface_id: "HuggingFaceTB/SmolLM2-1.7B-Instruct-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 1_700_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 24,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 1.1,
                    min_ram_gb: 1.5,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 1.9,
                    min_ram_gb: 2.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        // === Embedding Models ===
        CatalogEntry {
            name: "Nomic-Embed-Text-v1.5".into(),
            provider: "Nomic AI".into(),
            huggingface_id: "nomic-ai/nomic-embed-text-v1.5-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 137_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 12,
            available_quants: vec![QuantProfile {
                quant_type: "Q8_0".into(),
                file_size_gb: 0.15,
                min_ram_gb: 0.5,
                quality_rating: 0.99,
            }],
            specialization: ModelSpecialization::General,
        },
        // === DBRX ===
        CatalogEntry {
            name: "DBRX-Instruct-132B".into(),
            provider: "Databricks".into(),
            huggingface_id: "bartowski/dbrx-instruct-GGUF".into(),
            license: "Databricks Open".into(),
            total_params: 132_000_000_000,
            is_moe: true,
            active_params: Some(36_000_000_000),
            num_experts: Some(16),
            num_layers: 40,
            available_quants: vec![QuantProfile {
                quant_type: "Q4_K_M".into(),
                file_size_gb: 75.0,
                min_ram_gb: 44.0,
                quality_rating: 0.94,
            }],
            specialization: ModelSpecialization::General,
        },
        // === Command-R Series ===
        CatalogEntry {
            name: "Command-R-Plus-104B".into(),
            provider: "Cohere".into(),
            huggingface_id: "bartowski/c4ai-command-r-plus-GGUF".into(),
            license: "CC-BY-NC-4.0".into(),
            total_params: 104_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 64,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 60.0,
                    min_ram_gb: 36.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q2_K".into(),
                    file_size_gb: 37.0,
                    min_ram_gb: 22.0,
                    quality_rating: 0.82,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "Command-R-35B".into(),
            provider: "Cohere".into(),
            huggingface_id: "bartowski/c4ai-command-r-v01-GGUF".into(),
            license: "CC-BY-NC-4.0".into(),
            total_params: 35_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 40,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 20.0,
                    min_ram_gb: 12.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 36.0,
                    min_ram_gb: 20.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        // === Internlm ===
        CatalogEntry {
            name: "InternLM2.5-20B".into(),
            provider: "Shanghai AI Lab".into(),
            huggingface_id: "bartowski/internlm2_5-20b-chat-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 20_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 48,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 12.0,
                    min_ram_gb: 7.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 21.0,
                    min_ram_gb: 12.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
        CatalogEntry {
            name: "InternLM2.5-7B".into(),
            provider: "Shanghai AI Lab".into(),
            huggingface_id: "bartowski/internlm2_5-7b-chat-GGUF".into(),
            license: "Apache-2.0".into(),
            total_params: 7_000_000_000,
            is_moe: false,
            active_params: None,
            num_experts: None,
            num_layers: 32,
            available_quants: vec![
                QuantProfile {
                    quant_type: "Q4_K_M".into(),
                    file_size_gb: 4.4,
                    min_ram_gb: 3.0,
                    quality_rating: 0.94,
                },
                QuantProfile {
                    quant_type: "Q8_0".into(),
                    file_size_gb: 7.7,
                    min_ram_gb: 5.0,
                    quality_rating: 0.99,
                },
            ],
            specialization: ModelSpecialization::General,
        },
    ]
}
