//! Embedding abstraction for generating vector representations of memory content.
//!
//! The [`MemoryEmbedder`] trait allows different backends — local Ollama, cloud
//! providers, or the deterministic [`MockEmbedder`] for testing.
//!
//! The memory subsystem degrades gracefully when no embedder is available:
//! semantic search falls back to keyword matching.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Error type for embedding operations.
#[derive(Debug, thiserror::Error)]
pub enum EmbedError {
    /// The embedding provider is not reachable or not configured.
    #[error("Embedding provider unavailable: {0}")]
    ProviderUnavailable(String),
    /// The input text exceeds the model's context window.
    #[error("Text too long for embedding: {len} chars, max {max}")]
    TextTooLong { len: usize, max: usize },
    /// A catch-all for embedding failures.
    #[error("Embedding failed: {0}")]
    Failed(String),
}

/// Trait for embedding text into vector representations.
///
/// Implementations must be `Send + Sync` so they can live inside the
/// `MemoryManager` which is shared across async tasks.
pub trait MemoryEmbedder: Send + Sync {
    /// Generate an embedding vector for the given text.
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError>;

    /// Generate embeddings for multiple texts (batch).
    ///
    /// Default implementation calls [`embed`](Self::embed) in a loop.
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// The dimensionality of the embedding vectors produced by this embedder.
    fn dimension(&self) -> usize;

    /// Name of the embedding model or provider.
    fn model_name(&self) -> &str;
}

// ── Mock Embedder ────────────────────────────────────────────────────────────

/// Deterministic embedder for testing.
///
/// Produces 64-dimensional unit vectors derived from SHA-256 of the input text.
/// Identical inputs always produce identical vectors, and similar inputs produce
/// reasonably distinct vectors — good enough for testing ranking and retrieval.
#[derive(Debug, Clone)]
pub struct MockEmbedder;

impl MockEmbedder {
    /// Creates a new mock embedder.
    pub fn new() -> Self {
        Self
    }
}

impl Default for MockEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

const MOCK_DIM: usize = 64;

impl MemoryEmbedder for MockEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let hash = Sha256::digest(text.as_bytes());
        // Extend to 64 floats by hashing twice with different prefixes
        let hash2 = Sha256::digest([b"salt:", text.as_bytes()].concat());

        let mut vec = Vec::with_capacity(MOCK_DIM);
        for &byte in hash.iter().chain(hash2.iter()).take(MOCK_DIM) {
            vec.push(byte as f32 / 255.0);
        }

        // Normalize to unit vector
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vec {
                *v /= norm;
            }
        }

        Ok(vec)
    }

    fn dimension(&self) -> usize {
        MOCK_DIM
    }

    fn model_name(&self) -> &str {
        "mock-sha256-64d"
    }
}

// ── Ollama Embedder ──────────────────────────────────────────────────────────

/// Production embedder using a local Ollama instance.
///
/// Calls `POST {base_url}/api/embeddings` with the configured model.
/// Falls back gracefully with [`EmbedError::ProviderUnavailable`] when
/// Ollama is not running.
#[derive(Debug, Clone)]
pub struct OllamaEmbedder {
    base_url: String,
    model: String,
    dim: usize,
}

impl OllamaEmbedder {
    /// Creates an Ollama embedder with default settings.
    ///
    /// Defaults: `http://localhost:11434`, model `nomic-embed-text`, dim 768.
    pub fn new() -> Self {
        Self {
            base_url: "http://localhost:11434".into(),
            model: "nomic-embed-text".into(),
            dim: 768,
        }
    }

    /// Creates an Ollama embedder with custom settings.
    pub fn with_config(base_url: &str, model: &str, dimension: usize) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').into(),
            model: model.into(),
            dim: dimension,
        }
    }
}

impl Default for OllamaEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

/// Response shape from Ollama /api/embeddings.
#[derive(Deserialize)]
struct OllamaEmbedResponse {
    embedding: Vec<f32>,
}

/// Request body for Ollama /api/embeddings.
#[derive(Serialize)]
struct OllamaEmbedRequest<'a> {
    model: &'a str,
    prompt: &'a str,
}

impl MemoryEmbedder for OllamaEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let url = format!("{}/api/embeddings", self.base_url);
        let body = serde_json::to_string(&OllamaEmbedRequest {
            model: &self.model,
            prompt: text,
        })
        .map_err(|e| EmbedError::Failed(format!("serialize request: {e}")))?;

        // Use std::process::Command with curl for HTTP — no extra async deps needed,
        // consistent with how the codebase does lightweight HTTP calls.
        let output = std::process::Command::new("curl")
            .args([
                "-s",
                "--max-time",
                "30",
                "-X",
                "POST",
                &url,
                "-H",
                "Content-Type: application/json",
                "-d",
                &body,
            ])
            .output()
            .map_err(|e| EmbedError::ProviderUnavailable(format!("failed to run curl: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(EmbedError::ProviderUnavailable(format!(
                "curl failed (status {:?}): {stderr}",
                output.status.code()
            )));
        }

        let response_str = String::from_utf8_lossy(&output.stdout);
        if response_str.is_empty() {
            return Err(EmbedError::ProviderUnavailable(
                "empty response from Ollama".into(),
            ));
        }

        let resp: OllamaEmbedResponse = serde_json::from_str(&response_str).map_err(|e| {
            EmbedError::Failed(format!("parse response: {e} — body: {response_str}"))
        })?;

        Ok(resp.embedding)
    }

    fn dimension(&self) -> usize {
        self.dim
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_returns_consistent_vectors() {
        let embedder = MockEmbedder::new();
        let v1 = embedder.embed("hello world").unwrap();
        let v2 = embedder.embed("hello world").unwrap();
        assert_eq!(v1, v2);
    }

    #[test]
    fn mock_returns_different_vectors_for_different_input() {
        let embedder = MockEmbedder::new();
        let v1 = embedder.embed("hello world").unwrap();
        let v2 = embedder.embed("goodbye world").unwrap();
        assert_ne!(v1, v2);
    }

    #[test]
    fn mock_correct_dimension() {
        let embedder = MockEmbedder::new();
        let v = embedder.embed("test").unwrap();
        assert_eq!(v.len(), 64);
        assert_eq!(embedder.dimension(), 64);
    }

    #[test]
    fn mock_produces_unit_vector() {
        let embedder = MockEmbedder::new();
        let v = embedder.embed("anything").unwrap();
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "norm was {norm}, expected ~1.0");
    }

    #[test]
    fn mock_model_name() {
        let embedder = MockEmbedder::new();
        assert_eq!(embedder.model_name(), "mock-sha256-64d");
    }

    #[test]
    fn mock_batch_embed() {
        let embedder = MockEmbedder::new();
        let texts = &["alpha", "beta", "gamma"];
        let vecs = embedder.embed_batch(texts).unwrap();
        assert_eq!(vecs.len(), 3);
        // Each should be unique
        assert_ne!(vecs[0], vecs[1]);
        assert_ne!(vecs[1], vecs[2]);
    }

    #[test]
    fn ollama_construction() {
        let embedder = OllamaEmbedder::new();
        assert_eq!(embedder.model_name(), "nomic-embed-text");
        assert_eq!(embedder.dimension(), 768);
    }

    #[test]
    fn ollama_custom_config() {
        let embedder =
            OllamaEmbedder::with_config("http://gpu-server:11434/", "mxbai-embed-large", 1024);
        assert_eq!(embedder.base_url, "http://gpu-server:11434");
        assert_eq!(embedder.model_name(), "mxbai-embed-large");
        assert_eq!(embedder.dimension(), 1024);
    }
}
