//! In-memory vector store with cosine similarity search and JSON file persistence.

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

/// A stored embedding with metadata linking back to its source chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEmbedding {
    pub chunk_id: String,
    pub doc_path: String,
    pub chunk_index: usize,
    pub content: String,
    pub embedding: Vec<f32>,
}

/// A search result returned by similarity queries.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk_id: String,
    pub doc_path: String,
    pub chunk_index: usize,
    pub content: String,
    pub score: f32,
}

/// A 2D projected point for visualization of the embedding space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectedPoint {
    pub chunk_id: String,
    pub doc_path: String,
    pub x: f32,
    pub y: f32,
    pub label: String,
}

/// In-memory vector store backed by optional JSON file persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStore {
    embeddings: Vec<StoredEmbedding>,
    dimension: usize,
}

impl VectorStore {
    pub fn new(dimension: usize) -> Self {
        Self {
            embeddings: Vec::new(),
            dimension,
        }
    }

    /// Insert or update an embedding. Validates dimension. Replaces existing entry with same chunk_id.
    pub fn insert(&mut self, embedding: StoredEmbedding) -> Result<(), String> {
        if embedding.embedding.len() != self.dimension {
            return Err(format!(
                "dimension mismatch: expected {}, got {}",
                self.dimension,
                embedding.embedding.len()
            ));
        }
        if let Some(pos) = self
            .embeddings
            .iter()
            .position(|e| e.chunk_id == embedding.chunk_id)
        {
            self.embeddings[pos] = embedding;
        } else {
            self.embeddings.push(embedding);
        }
        Ok(())
    }

    /// Search for the top_k most similar embeddings to the query vector.
    pub fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<SearchResult>, String> {
        if query_embedding.len() != self.dimension {
            return Err(format!(
                "dimension mismatch: expected {}, got {}",
                self.dimension,
                query_embedding.len()
            ));
        }

        let mut scored: Vec<(&StoredEmbedding, f32)> = self
            .embeddings
            .iter()
            .map(|e| (e, cosine_similarity(query_embedding, &e.embedding)))
            .filter(|(_, score)| *score > 0.0)
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);

        Ok(scored
            .into_iter()
            .map(|(e, score)| SearchResult {
                chunk_id: e.chunk_id.clone(),
                doc_path: e.doc_path.clone(),
                chunk_index: e.chunk_index,
                content: e.content.clone(),
                score,
            })
            .collect())
    }

    /// Remove all embeddings for a given document path. Returns count removed.
    pub fn remove_document(&mut self, doc_path: &str) -> usize {
        let before = self.embeddings.len();
        self.embeddings.retain(|e| e.doc_path != doc_path);
        before - self.embeddings.len()
    }

    /// Count unique document paths.
    pub fn document_count(&self) -> usize {
        self.embeddings
            .iter()
            .map(|e| &e.doc_path)
            .collect::<BTreeSet<_>>()
            .len()
    }

    /// Total stored embeddings.
    pub fn total_embeddings(&self) -> usize {
        self.embeddings.len()
    }

    /// Return sorted unique document paths.
    pub fn list_documents(&self) -> Vec<String> {
        self.embeddings
            .iter()
            .map(|e| e.doc_path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Project all embeddings to 2D using random projection (Johnson-Lindenstrauss).
    pub fn get_2d_projection(&self) -> Vec<ProjectedPoint> {
        if self.embeddings.is_empty() || self.dimension == 0 {
            return Vec::new();
        }

        // Generate two deterministic projection vectors using seeded hashing.
        let proj_x = make_projection_vector(self.dimension, 0xCAFE_0001);
        let proj_y = make_projection_vector(self.dimension, 0xCAFE_0002);

        // Project each embedding onto the two vectors.
        let points: Vec<(String, String, f32, f32, String)> = self
            .embeddings
            .iter()
            .map(|e| {
                let x = dot(&e.embedding, &proj_x);
                let y = dot(&e.embedding, &proj_y);
                let label: String = e.content.chars().take(50).collect();
                (e.chunk_id.clone(), e.doc_path.clone(), x, y, label)
            })
            .collect();

        // Normalize x, y to [-1.0, 1.0].
        let (min_x, max_x, min_y, max_y) = points.iter().fold(
            (f32::MAX, f32::MIN, f32::MAX, f32::MIN),
            |(mnx, mxx, mny, mxy), (_, _, x, y, _)| {
                (mnx.min(*x), mxx.max(*x), mny.min(*y), mxy.max(*y))
            },
        );

        let range_x = max_x - min_x;
        let range_y = max_y - min_y;

        points
            .into_iter()
            .map(|(chunk_id, doc_path, x, y, label)| {
                let nx = if range_x > 0.0 {
                    (x - min_x) / range_x * 2.0 - 1.0
                } else {
                    0.0
                };
                let ny = if range_y > 0.0 {
                    (y - min_y) / range_y * 2.0 - 1.0
                } else {
                    0.0
                };
                ProjectedPoint {
                    chunk_id,
                    doc_path,
                    x: nx,
                    y: ny,
                    label,
                }
            })
            .collect()
    }

    /// Serialize the store to a JSON file.
    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string(self).map_err(|e| format!("serialize error: {e}"))?;
        std::fs::write(path, json).map_err(|e| format!("write error: {e}"))
    }

    /// Load a store from a JSON file. Returns an empty store if the file doesn't exist.
    pub fn load_from_file(path: &str) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(json) => serde_json::from_str(&json).map_err(|e| format!("parse error: {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::new(0)),
            Err(e) => Err(format!("read error: {e}")),
        }
    }
}

/// Compute cosine similarity between two vectors: dot(a,b) / (‖a‖ × ‖b‖).
/// Returns 0.0 if either vector has zero norm.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let d: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    d / (norm_a * norm_b)
}

/// Dot product of two slices.
fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Generate a deterministic pseudo-random projection vector of the given dimension.
fn make_projection_vector(dimension: usize, seed: u64) -> Vec<f32> {
    (0..dimension)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            seed.hash(&mut hasher);
            i.hash(&mut hasher);
            let h = hasher.finish();
            // Map hash to [-1.0, 1.0].
            (h as f64 / u64::MAX as f64) as f32 * 2.0 - 1.0
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_embedding(
        chunk_id: &str,
        doc_path: &str,
        chunk_index: usize,
        embedding: Vec<f32>,
    ) -> StoredEmbedding {
        StoredEmbedding {
            chunk_id: chunk_id.to_string(),
            doc_path: doc_path.to_string(),
            chunk_index,
            content: format!("content of {chunk_id}"),
            embedding,
        }
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let score = cosine_similarity(&v, &v);
        assert!((score - 1.0).abs() < 1e-6, "expected ~1.0, got {score}");
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let score = cosine_similarity(&a, &b);
        assert!(score.abs() < 1e-6, "expected ~0.0, got {score}");
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        let score = cosine_similarity(&a, &b);
        assert!((score + 1.0).abs() < 1e-6, "expected ~-1.0, got {score}");
    }

    #[test]
    fn test_insert_and_search() {
        let mut store = VectorStore::new(3);
        store
            .insert(make_embedding("c1", "doc1.md", 0, vec![1.0, 0.0, 0.0]))
            .unwrap();
        store
            .insert(make_embedding("c2", "doc1.md", 1, vec![0.0, 1.0, 0.0]))
            .unwrap();
        store
            .insert(make_embedding("c3", "doc2.md", 0, vec![0.9, 0.1, 0.0]))
            .unwrap();

        let results = store.search(&[1.0, 0.0, 0.0], 3).unwrap();
        assert_eq!(results[0].chunk_id, "c1");
        assert_eq!(results[1].chunk_id, "c3");
    }

    #[test]
    fn test_search_top_k() {
        let mut store = VectorStore::new(3);
        for i in 0..10 {
            let v = vec![i as f32, 1.0, 1.0];
            store
                .insert(make_embedding(&format!("c{i}"), "doc.md", i, v))
                .unwrap();
        }
        let results = store.search(&[10.0, 1.0, 1.0], 3).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_upsert_behavior() {
        let mut store = VectorStore::new(3);
        store
            .insert(make_embedding("c1", "doc.md", 0, vec![1.0, 0.0, 0.0]))
            .unwrap();
        store
            .insert(make_embedding("c1", "doc.md", 0, vec![0.0, 1.0, 0.0]))
            .unwrap();
        assert_eq!(store.total_embeddings(), 1);
        let results = store.search(&[0.0, 1.0, 0.0], 1).unwrap();
        assert_eq!(results[0].chunk_id, "c1");
        assert!((results[0].score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_remove_document() {
        let mut store = VectorStore::new(3);
        store
            .insert(make_embedding("c1", "doc1.md", 0, vec![1.0, 0.0, 0.0]))
            .unwrap();
        store
            .insert(make_embedding("c2", "doc1.md", 1, vec![0.0, 1.0, 0.0]))
            .unwrap();
        store
            .insert(make_embedding("c3", "doc2.md", 0, vec![0.0, 0.0, 1.0]))
            .unwrap();
        let removed = store.remove_document("doc1.md");
        assert_eq!(removed, 2);
        assert_eq!(store.total_embeddings(), 1);
        assert_eq!(store.list_documents(), vec!["doc2.md".to_string()]);
    }

    #[test]
    fn test_dimension_mismatch_insert() {
        let mut store = VectorStore::new(3);
        let result = store.insert(make_embedding("c1", "doc.md", 0, vec![1.0, 2.0]));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("dimension mismatch"));
    }

    #[test]
    fn test_dimension_mismatch_search() {
        let store = VectorStore::new(3);
        let result = store.search(&[1.0, 2.0], 5);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("dimension mismatch"));
    }

    #[test]
    fn test_document_count() {
        let mut store = VectorStore::new(3);
        store
            .insert(make_embedding("c1", "doc1.md", 0, vec![1.0, 0.0, 0.0]))
            .unwrap();
        store
            .insert(make_embedding("c2", "doc2.md", 0, vec![0.0, 1.0, 0.0]))
            .unwrap();
        store
            .insert(make_embedding("c3", "doc3.md", 0, vec![0.0, 0.0, 1.0]))
            .unwrap();
        store
            .insert(make_embedding("c4", "doc1.md", 1, vec![1.0, 1.0, 0.0]))
            .unwrap();
        assert_eq!(store.document_count(), 3);
    }

    #[test]
    fn test_list_documents() {
        let mut store = VectorStore::new(3);
        store
            .insert(make_embedding("c1", "zebra.md", 0, vec![1.0, 0.0, 0.0]))
            .unwrap();
        store
            .insert(make_embedding("c2", "alpha.md", 0, vec![0.0, 1.0, 0.0]))
            .unwrap();
        store
            .insert(make_embedding("c3", "middle.md", 0, vec![0.0, 0.0, 1.0]))
            .unwrap();
        assert_eq!(
            store.list_documents(),
            vec![
                "alpha.md".to_string(),
                "middle.md".to_string(),
                "zebra.md".to_string()
            ]
        );
    }

    #[test]
    fn test_save_and_load() {
        let mut store = VectorStore::new(3);
        store
            .insert(make_embedding("c1", "doc1.md", 0, vec![1.0, 0.0, 0.0]))
            .unwrap();
        store
            .insert(make_embedding("c2", "doc2.md", 0, vec![0.0, 1.0, 0.0]))
            .unwrap();

        let path = std::env::temp_dir().join("nexus_vector_store_test.json");
        let path_str = path.to_str().unwrap();

        store.save_to_file(path_str).unwrap();
        let loaded = VectorStore::load_from_file(path_str).unwrap();

        assert_eq!(loaded.dimension, store.dimension);
        assert_eq!(loaded.total_embeddings(), store.total_embeddings());
        assert_eq!(loaded.list_documents(), store.list_documents());

        // Verify search produces same results
        let orig_results = store.search(&[1.0, 0.0, 0.0], 2).unwrap();
        let loaded_results = loaded.search(&[1.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(orig_results[0].chunk_id, loaded_results[0].chunk_id);

        let _ = std::fs::remove_file(path_str);
    }

    #[test]
    fn test_load_nonexistent() {
        let store = VectorStore::load_from_file("/tmp/nexus_no_such_file_999.json").unwrap();
        assert_eq!(store.total_embeddings(), 0);
    }

    #[test]
    fn test_empty_store_search() {
        let store = VectorStore::new(3);
        let results = store.search(&[1.0, 0.0, 0.0], 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_2d_projection_returns_all_points() {
        let mut store = VectorStore::new(3);
        for i in 0..5 {
            store
                .insert(make_embedding(
                    &format!("c{i}"),
                    "doc.md",
                    i,
                    vec![i as f32, 1.0, 2.0],
                ))
                .unwrap();
        }
        let points = store.get_2d_projection();
        assert_eq!(points.len(), 5);
    }

    #[test]
    fn test_2d_projection_normalized() {
        let mut store = VectorStore::new(3);
        for i in 0..10 {
            store
                .insert(make_embedding(
                    &format!("c{i}"),
                    "doc.md",
                    i,
                    vec![i as f32 * 100.0, -50.0, 75.0],
                ))
                .unwrap();
        }
        let points = store.get_2d_projection();
        for p in &points {
            assert!(p.x >= -1.0 && p.x <= 1.0, "x out of range: {}", p.x);
            assert!(p.y >= -1.0 && p.y <= 1.0, "y out of range: {}", p.y);
        }
    }

    #[test]
    fn test_2d_projection_deterministic() {
        let mut store = VectorStore::new(3);
        for i in 0..5 {
            store
                .insert(make_embedding(
                    &format!("c{i}"),
                    "doc.md",
                    i,
                    vec![i as f32, 1.0, 2.0],
                ))
                .unwrap();
        }
        let proj1 = store.get_2d_projection();
        let proj2 = store.get_2d_projection();
        for (a, b) in proj1.iter().zip(proj2.iter()) {
            assert!((a.x - b.x).abs() < 1e-6);
            assert!((a.y - b.y).abs() < 1e-6);
        }
    }

    #[test]
    fn test_2d_projection_empty() {
        let store = VectorStore::new(3);
        let points = store.get_2d_projection();
        assert!(points.is_empty());
    }
}
