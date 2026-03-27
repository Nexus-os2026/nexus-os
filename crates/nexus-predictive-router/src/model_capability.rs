//! Model capability profiles — every available model gets a measured profile
//! with the same vector dimensions as agent measurement.

use nexus_capability_measurement::framework::{DifficultyLevel, Vector};
use serde::{Deserialize, Serialize};

/// A model's capability profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilityProfile {
    pub model_id: String,
    pub provider: String,
    pub display_name: String,
    pub vector_scores: VectorScores,
    pub vector_ceilings: VectorCeilings,
    pub cost_per_1k_input: f64,
    pub cost_per_1k_output: f64,
    pub avg_latency_ms: u64,
    pub available: bool,
    pub is_local: bool,
    pub size_class: ModelSizeClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ModelSizeClass {
    Tiny,
    Small,
    Medium,
    Large,
    Frontier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorScores {
    pub reasoning_depth: f64,
    pub planning_coherence: f64,
    pub adaptation: f64,
    pub tool_use: f64,
}

impl VectorScores {
    pub fn score_for(&self, vector: Vector) -> f64 {
        match vector {
            Vector::ReasoningDepth => self.reasoning_depth,
            Vector::PlanningCoherence => self.planning_coherence,
            Vector::AdaptationUnderUncertainty => self.adaptation,
            Vector::ToolUseIntegrity => self.tool_use,
        }
    }

    pub fn floor(&self) -> f64 {
        [
            self.reasoning_depth,
            self.planning_coherence,
            self.adaptation,
            self.tool_use,
        ]
        .into_iter()
        .fold(f64::MAX, f64::min)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorCeilings {
    pub reasoning_depth: Option<DifficultyLevel>,
    pub planning_coherence: Option<DifficultyLevel>,
    pub adaptation: Option<DifficultyLevel>,
    pub tool_use: Option<DifficultyLevel>,
}

/// Registry of all available models with capability profiles.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelRegistry {
    pub models: Vec<ModelCapabilityProfile>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, profile: ModelCapabilityProfile) {
        if let Some(existing) = self
            .models
            .iter_mut()
            .find(|m| m.model_id == profile.model_id)
        {
            *existing = profile;
        } else {
            self.models.push(profile);
        }
    }

    /// Get available models sorted by capability (ascending) for a vector.
    pub fn models_for_vector(&self, vector: Vector) -> Vec<&ModelCapabilityProfile> {
        let mut available: Vec<&ModelCapabilityProfile> =
            self.models.iter().filter(|m| m.available).collect();
        available.sort_by(|a, b| {
            a.vector_scores
                .score_for(vector)
                .partial_cmp(&b.vector_scores.score_for(vector))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        available
    }

    /// Get the cheapest model that meets a minimum capability threshold.
    pub fn cheapest_above_threshold(
        &self,
        vector: Vector,
        min_score: f64,
    ) -> Option<&ModelCapabilityProfile> {
        self.models
            .iter()
            .filter(|m| m.available && m.vector_scores.score_for(vector) >= min_score)
            .min_by(|a, b| {
                a.cost_per_1k_input
                    .partial_cmp(&b.cost_per_1k_input)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

/// Build a default model registry with estimated capability profiles
/// for known model tiers (local + cloud).
pub fn default_model_registry() -> ModelRegistry {
    let mut reg = ModelRegistry::new();

    // ── Local models (Flash Inference / Ollama) — free, low latency ──
    reg.register(ModelCapabilityProfile {
        model_id: "flash-2b".into(),
        provider: "flash".into(),
        display_name: "Flash 2B".into(),
        vector_scores: VectorScores {
            reasoning_depth: 0.25,
            planning_coherence: 0.20,
            adaptation: 0.15,
            tool_use: 0.20,
        },
        vector_ceilings: VectorCeilings {
            reasoning_depth: Some(DifficultyLevel::Level1),
            planning_coherence: Some(DifficultyLevel::Level1),
            adaptation: Some(DifficultyLevel::Level1),
            tool_use: Some(DifficultyLevel::Level1),
        },
        cost_per_1k_input: 0.0,
        cost_per_1k_output: 0.0,
        avg_latency_ms: 30,
        available: true,
        is_local: true,
        size_class: ModelSizeClass::Tiny,
    });
    reg.register(ModelCapabilityProfile {
        model_id: "ollama-7b".into(),
        provider: "ollama".into(),
        display_name: "Ollama 7B".into(),
        vector_scores: VectorScores {
            reasoning_depth: 0.45,
            planning_coherence: 0.40,
            adaptation: 0.35,
            tool_use: 0.40,
        },
        vector_ceilings: VectorCeilings {
            reasoning_depth: Some(DifficultyLevel::Level2),
            planning_coherence: Some(DifficultyLevel::Level2),
            adaptation: Some(DifficultyLevel::Level2),
            tool_use: Some(DifficultyLevel::Level2),
        },
        cost_per_1k_input: 0.0,
        cost_per_1k_output: 0.0,
        avg_latency_ms: 80,
        available: true,
        is_local: true,
        size_class: ModelSizeClass::Small,
    });
    reg.register(ModelCapabilityProfile {
        model_id: "flash-35b".into(),
        provider: "flash".into(),
        display_name: "Flash 35B".into(),
        vector_scores: VectorScores {
            reasoning_depth: 0.65,
            planning_coherence: 0.60,
            adaptation: 0.55,
            tool_use: 0.60,
        },
        vector_ceilings: VectorCeilings {
            reasoning_depth: Some(DifficultyLevel::Level3),
            planning_coherence: Some(DifficultyLevel::Level3),
            adaptation: Some(DifficultyLevel::Level3),
            tool_use: Some(DifficultyLevel::Level3),
        },
        cost_per_1k_input: 0.0,
        cost_per_1k_output: 0.0,
        avg_latency_ms: 200,
        available: true,
        is_local: true,
        size_class: ModelSizeClass::Medium,
    });

    // ── Cloud models (NIM, benchmark-validated) ──
    reg.register(ModelCapabilityProfile {
        model_id: "nim-mistral-7b".into(),
        provider: "nvidia_nim".into(),
        display_name: "Mistral 7B (NIM)".into(),
        vector_scores: VectorScores {
            reasoning_depth: 0.55,
            planning_coherence: 0.50,
            adaptation: 0.45,
            tool_use: 0.55,
        },
        vector_ceilings: VectorCeilings {
            reasoning_depth: Some(DifficultyLevel::Level3),
            planning_coherence: Some(DifficultyLevel::Level2),
            adaptation: Some(DifficultyLevel::Level2),
            tool_use: Some(DifficultyLevel::Level3),
        },
        cost_per_1k_input: 0.001,
        cost_per_1k_output: 0.002,
        avg_latency_ms: 224,
        available: true,
        is_local: false,
        size_class: ModelSizeClass::Small,
    });
    reg.register(ModelCapabilityProfile {
        model_id: "nim-llama-70b".into(),
        provider: "nvidia_nim".into(),
        display_name: "Llama 3.1 70B (NIM)".into(),
        vector_scores: VectorScores {
            reasoning_depth: 0.80,
            planning_coherence: 0.75,
            adaptation: 0.70,
            tool_use: 0.75,
        },
        vector_ceilings: VectorCeilings {
            reasoning_depth: Some(DifficultyLevel::Level4),
            planning_coherence: Some(DifficultyLevel::Level4),
            adaptation: Some(DifficultyLevel::Level3),
            tool_use: Some(DifficultyLevel::Level4),
        },
        cost_per_1k_input: 0.003,
        cost_per_1k_output: 0.006,
        avg_latency_ms: 398,
        available: true,
        is_local: false,
        size_class: ModelSizeClass::Large,
    });

    // ── Frontier cloud models ──
    reg.register(ModelCapabilityProfile {
        model_id: "claude-sonnet".into(),
        provider: "anthropic".into(),
        display_name: "Claude Sonnet 4.5".into(),
        vector_scores: VectorScores {
            reasoning_depth: 0.92,
            planning_coherence: 0.90,
            adaptation: 0.88,
            tool_use: 0.90,
        },
        vector_ceilings: VectorCeilings {
            reasoning_depth: Some(DifficultyLevel::Level5),
            planning_coherence: Some(DifficultyLevel::Level5),
            adaptation: Some(DifficultyLevel::Level4),
            tool_use: Some(DifficultyLevel::Level5),
        },
        cost_per_1k_input: 0.003,
        cost_per_1k_output: 0.015,
        avg_latency_ms: 800,
        available: true,
        is_local: false,
        size_class: ModelSizeClass::Frontier,
    });

    reg
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_model(id: &str, score: f64, cost: f64) -> ModelCapabilityProfile {
        ModelCapabilityProfile {
            model_id: id.into(),
            provider: "test".into(),
            display_name: id.into(),
            vector_scores: VectorScores {
                reasoning_depth: score,
                planning_coherence: score,
                adaptation: score,
                tool_use: score,
            },
            vector_ceilings: VectorCeilings {
                reasoning_depth: None,
                planning_coherence: None,
                adaptation: None,
                tool_use: None,
            },
            cost_per_1k_input: cost,
            cost_per_1k_output: cost * 2.0,
            avg_latency_ms: 100,
            available: true,
            is_local: false,
            size_class: ModelSizeClass::Medium,
        }
    }

    #[test]
    fn test_model_registry_sorted_by_capability() {
        let mut reg = ModelRegistry::new();
        reg.register(make_model("high", 0.9, 1.0));
        reg.register(make_model("low", 0.3, 0.1));
        reg.register(make_model("mid", 0.6, 0.5));

        let sorted = reg.models_for_vector(Vector::ReasoningDepth);
        assert_eq!(sorted[0].model_id, "low");
        assert_eq!(sorted[1].model_id, "mid");
        assert_eq!(sorted[2].model_id, "high");
    }

    #[test]
    fn test_cheapest_above_threshold() {
        let mut reg = ModelRegistry::new();
        reg.register(make_model("expensive-good", 0.9, 5.0));
        reg.register(make_model("cheap-good", 0.8, 1.0));
        reg.register(make_model("cheap-bad", 0.3, 0.5));

        let best = reg
            .cheapest_above_threshold(Vector::ReasoningDepth, 0.7)
            .unwrap();
        assert_eq!(best.model_id, "cheap-good");
    }

    #[test]
    fn test_vector_scores_floor() {
        let scores = VectorScores {
            reasoning_depth: 0.8,
            planning_coherence: 0.6,
            adaptation: 0.9,
            tool_use: 0.5,
        };
        assert!((scores.floor() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_default_model_registry_has_models() {
        let reg = default_model_registry();
        assert!(
            reg.models.len() >= 5,
            "Default registry should have at least 5 models"
        );
        // Should have both local and cloud models
        assert!(reg.models.iter().any(|m| m.is_local));
        assert!(reg.models.iter().any(|m| !m.is_local));
        // Should span size classes
        assert!(reg
            .models
            .iter()
            .any(|m| m.size_class == ModelSizeClass::Tiny));
        assert!(reg
            .models
            .iter()
            .any(|m| m.size_class == ModelSizeClass::Frontier));
    }
}
