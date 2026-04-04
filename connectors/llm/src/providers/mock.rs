use super::{EmbeddingResponse, LlmProvider, LlmResponse};
use nexus_kernel::errors::AgentError;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const MOCK_EMBEDDING_DIM: usize = 384;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MockProvider;

impl MockProvider {
    pub fn new() -> Self {
        Self
    }
}

/// Generate a deterministic, normalized embedding vector from text.
fn mock_embed_text(text: &str) -> Vec<f32> {
    let mut raw = Vec::with_capacity(MOCK_EMBEDDING_DIM);
    for i in 0..MOCK_EMBEDDING_DIM {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        i.hash(&mut hasher);
        let hash = hasher.finish();
        // Map hash to [-1, 1] range.
        raw.push((hash as f64 / u64::MAX as f64) * 2.0 - 1.0);
    }

    // L2-normalize so cosine similarity works correctly.
    let norm: f64 = raw.iter().map(|v| v * v).sum::<f64>().sqrt();
    if norm > 0.0 {
        raw.iter().map(|v| (v / norm) as f32).collect()
    } else {
        vec![0.0f32; MOCK_EMBEDDING_DIM]
    }
}

impl LlmProvider for MockProvider {
    fn query(
        &self,
        _prompt: &str,
        max_tokens: u32,
        model: &str,
    ) -> Result<LlmResponse, AgentError> {
        Ok(LlmResponse {
            output_text: "[Mock Response - No LLM configured] Configure a provider in Settings to get real responses.".to_string(),
            token_count: max_tokens.min(64),
            model_name: model.to_string(),
            tool_calls: Vec::new(),
            input_tokens: None,
        })
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn cost_per_token(&self) -> f64 {
        0.0
    }

    fn embed(&self, texts: &[&str], model: &str) -> Result<EmbeddingResponse, AgentError> {
        let embeddings: Vec<Vec<f32>> = texts.iter().map(|text| mock_embed_text(text)).collect();
        let token_count = texts
            .iter()
            .map(|t| t.split_whitespace().count() as u32)
            .sum();
        Ok(EmbeddingResponse {
            embeddings,
            model_name: model.to_string(),
            token_count,
        })
    }
}
