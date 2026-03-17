//! Knowledge graph — auto-links files by shared semantics, references, and temporal proximity.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::indexer::{IndexedFile, SemanticIndexer};

/// The type of relationship between two files.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LinkType {
    /// Both files contain the same named entity.
    SharedEntity,
    /// Both files share one or more top topics.
    SharedTopic,
    /// Both files were modified/indexed within a short time window.
    TemporalProximity,
    /// Files have similar word frequency distributions.
    SemanticSimilarity,
    /// One file explicitly references the other's path.
    ExplicitReference,
}

/// A directed edge in the knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphLink {
    /// Unique link identifier.
    pub id: Uuid,
    /// Source file path.
    pub source: String,
    /// Target file path.
    pub target: String,
    /// What connects these files.
    pub link_type: LinkType,
    /// Strength of the connection (0.0 to 1.0).
    pub strength: f64,
    /// When the link was detected.
    pub detected_at: DateTime<Utc>,
}

/// A knowledge graph over the indexed file corpus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    /// Adjacency list: path -> set of links originating from that path.
    adjacency: HashMap<String, Vec<GraphLink>>,
    /// All known file paths in the graph.
    nodes: HashSet<String>,
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeGraph {
    /// Create a new empty knowledge graph.
    pub fn new() -> Self {
        Self {
            adjacency: HashMap::new(),
            nodes: HashSet::new(),
        }
    }

    /// Add a file to the graph and auto-detect links to existing files.
    ///
    /// Uses the indexer to compare against all other indexed files.
    pub fn add_file(&mut self, file: &IndexedFile, indexer: &SemanticIndexer) {
        self.nodes.insert(file.path.clone());

        // Compare against every other indexed file to find links
        for (other_path, other) in &indexer.index {
            if *other_path == file.path {
                continue;
            }
            let links = Self::detect_links(file, other);
            for link in links {
                // Add forward link
                self.adjacency
                    .entry(link.source.clone())
                    .or_default()
                    .push(link.clone());

                // Add reverse link
                let reverse = GraphLink {
                    id: Uuid::new_v4(),
                    source: link.target.clone(),
                    target: link.source.clone(),
                    link_type: link.link_type.clone(),
                    strength: link.strength,
                    detected_at: link.detected_at,
                };
                self.adjacency
                    .entry(reverse.source.clone())
                    .or_default()
                    .push(reverse);
            }
        }
    }

    /// Remove a file and all its links from the graph.
    pub fn remove_file(&mut self, path: &str) {
        self.nodes.remove(path);
        self.adjacency.remove(path);

        // Remove links pointing to this file from other nodes
        for links in self.adjacency.values_mut() {
            links.retain(|l| l.target != path);
        }
    }

    /// Get all links originating from a path.
    pub fn get_links(&self, path: &str) -> Vec<&GraphLink> {
        self.adjacency
            .get(path)
            .map(|links| links.iter().collect())
            .unwrap_or_default()
    }

    /// Find the most related files to a given path, ranked by total link strength.
    pub fn find_related(&self, path: &str, limit: usize) -> Vec<(String, f64)> {
        let links = match self.adjacency.get(path) {
            Some(l) => l,
            None => return Vec::new(),
        };

        // Aggregate strength per target
        let mut scores: HashMap<&str, f64> = HashMap::new();
        for link in links {
            *scores.entry(&link.target).or_insert(0.0) += link.strength;
        }

        let mut ranked: Vec<(String, f64)> = scores
            .into_iter()
            .map(|(p, s)| (p.to_string(), s))
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked.truncate(limit);
        ranked
    }

    /// Return all node paths in the graph.
    pub fn nodes(&self) -> &HashSet<String> {
        &self.nodes
    }

    /// Total number of links in the graph.
    pub fn link_count(&self) -> usize {
        self.adjacency.values().map(|v| v.len()).sum()
    }

    /// Detect links between two indexed files.
    fn detect_links(a: &IndexedFile, b: &IndexedFile) -> Vec<GraphLink> {
        let mut links = Vec::new();
        let now = Utc::now();

        // Shared entities
        let a_entities: HashSet<&String> = a.entities.iter().collect();
        let b_entities: HashSet<&String> = b.entities.iter().collect();
        let shared_entities: HashSet<&&String> = a_entities.intersection(&b_entities).collect();
        if !shared_entities.is_empty() {
            let strength = (shared_entities.len() as f64
                / a_entities.len().max(b_entities.len()).max(1) as f64)
                .min(1.0);
            links.push(GraphLink {
                id: Uuid::new_v4(),
                source: a.path.clone(),
                target: b.path.clone(),
                link_type: LinkType::SharedEntity,
                strength,
                detected_at: now,
            });
        }

        // Shared topics
        let a_topics: HashSet<&String> = a.topics.iter().collect();
        let b_topics: HashSet<&String> = b.topics.iter().collect();
        let shared_topics: HashSet<&&String> = a_topics.intersection(&b_topics).collect();
        if !shared_topics.is_empty() {
            let strength = (shared_topics.len() as f64
                / a_topics.len().max(b_topics.len()).max(1) as f64)
                .min(1.0);
            links.push(GraphLink {
                id: Uuid::new_v4(),
                source: a.path.clone(),
                target: b.path.clone(),
                link_type: LinkType::SharedTopic,
                strength,
                detected_at: now,
            });
        }

        // Temporal proximity (indexed within 1 hour of each other)
        let time_diff = (a.indexed_at - b.indexed_at).num_seconds().unsigned_abs();
        if time_diff < 3600 {
            let strength = 1.0 - (time_diff as f64 / 3600.0);
            links.push(GraphLink {
                id: Uuid::new_v4(),
                source: a.path.clone(),
                target: b.path.clone(),
                link_type: LinkType::TemporalProximity,
                strength,
                detected_at: now,
            });
        }

        // Semantic similarity via cosine similarity of word frequencies
        let sim = Self::cosine_similarity(&a.word_frequencies, &b.word_frequencies);
        if sim > 0.1 {
            links.push(GraphLink {
                id: Uuid::new_v4(),
                source: a.path.clone(),
                target: b.path.clone(),
                link_type: LinkType::SemanticSimilarity,
                strength: sim,
                detected_at: now,
            });
        }

        // Explicit references
        if a.file_references.iter().any(|r| r.contains(&b.path))
            || a.file_references.iter().any(|r| b.path.ends_with(r))
        {
            links.push(GraphLink {
                id: Uuid::new_v4(),
                source: a.path.clone(),
                target: b.path.clone(),
                link_type: LinkType::ExplicitReference,
                strength: 1.0,
                detected_at: now,
            });
        }

        links
    }

    /// Cosine similarity between two word frequency vectors.
    fn cosine_similarity(a: &HashMap<String, u32>, b: &HashMap<String, u32>) -> f64 {
        let all_words: HashSet<&String> = a.keys().chain(b.keys()).collect();
        if all_words.is_empty() {
            return 0.0;
        }

        let mut dot = 0.0_f64;
        let mut mag_a = 0.0_f64;
        let mut mag_b = 0.0_f64;

        for word in &all_words {
            let va = *a.get(*word).unwrap_or(&0) as f64;
            let vb = *b.get(*word).unwrap_or(&0) as f64;
            dot += va * vb;
            mag_a += va * va;
            mag_b += vb * vb;
        }

        let denom = mag_a.sqrt() * mag_b.sqrt();
        if denom == 0.0 {
            0.0
        } else {
            dot / denom
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_indexer_with_files() -> (SemanticIndexer, IndexedFile, IndexedFile) {
        let mut indexer = SemanticIndexer::new();
        let a = indexer
            .index_file(
                "project/readme.md",
                "Rust programming systems design. John Smith wrote this on 2024-06-15.",
                70,
            )
            .unwrap();
        let b = indexer
            .index_file(
                "project/design.md",
                "Systems design patterns in Rust. Alice Johnson reviewed on 2024-06-15.",
                72,
            )
            .unwrap();
        (indexer, a, b)
    }

    #[test]
    fn test_add_file_and_get_links() {
        let (indexer, a, b) = make_indexer_with_files();
        let mut graph = KnowledgeGraph::new();
        graph.add_file(&a, &indexer);
        graph.add_file(&b, &indexer);

        let links = graph.get_links("project/readme.md");
        assert!(!links.is_empty());
        // Should have shared topic links (both mention rust, systems, design)
        assert!(links.iter().any(|l| l.link_type == LinkType::SharedTopic));
    }

    #[test]
    fn test_shared_entity_detection() {
        let (indexer, a, b) = make_indexer_with_files();
        let mut graph = KnowledgeGraph::new();
        graph.add_file(&a, &indexer);
        graph.add_file(&b, &indexer);

        let links = graph.get_links("project/readme.md");
        // Both share the date entity 2024-06-15
        assert!(links.iter().any(|l| l.link_type == LinkType::SharedEntity));
    }

    #[test]
    fn test_remove_file() {
        let (indexer, a, b) = make_indexer_with_files();
        let mut graph = KnowledgeGraph::new();
        graph.add_file(&a, &indexer);
        graph.add_file(&b, &indexer);

        graph.remove_file("project/readme.md");
        assert!(!graph.nodes().contains("project/readme.md"));
        assert!(graph.get_links("project/readme.md").is_empty());
        // Links from design.md to readme.md should also be gone
        let design_links = graph.get_links("project/design.md");
        assert!(design_links.iter().all(|l| l.target != "project/readme.md"));
    }

    #[test]
    fn test_find_related() {
        let (indexer, a, b) = make_indexer_with_files();
        let mut graph = KnowledgeGraph::new();
        graph.add_file(&a, &indexer);
        graph.add_file(&b, &indexer);

        let related = graph.find_related("project/readme.md", 5);
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].0, "project/design.md");
        assert!(related[0].1 > 0.0);
    }

    #[test]
    fn test_cosine_similarity() {
        let a: HashMap<String, u32> = [("rust".into(), 3), ("code".into(), 2)]
            .into_iter()
            .collect();
        let b: HashMap<String, u32> = [("rust".into(), 2), ("code".into(), 1)]
            .into_iter()
            .collect();
        let sim = KnowledgeGraph::cosine_similarity(&a, &b);
        assert!(sim > 0.9); // very similar vectors

        let c: HashMap<String, u32> = [("java".into(), 5)].into_iter().collect();
        let sim2 = KnowledgeGraph::cosine_similarity(&a, &c);
        assert!((sim2 - 0.0).abs() < f64::EPSILON); // completely different
    }

    #[test]
    fn test_temporal_proximity() {
        let mut indexer = SemanticIndexer::new();
        // Index two files almost simultaneously — they should get temporal proximity links
        let a = indexer.index_file("a.txt", "unique alpha", 12).unwrap();
        let b = indexer.index_file("b.txt", "unique beta", 11).unwrap();

        let mut graph = KnowledgeGraph::new();
        graph.add_file(&a, &indexer);
        graph.add_file(&b, &indexer);

        let links = graph.get_links("a.txt");
        assert!(links
            .iter()
            .any(|l| l.link_type == LinkType::TemporalProximity));
    }

    #[test]
    fn test_empty_graph() {
        let graph = KnowledgeGraph::new();
        assert!(graph.nodes().is_empty());
        assert_eq!(graph.link_count(), 0);
        assert!(graph.find_related("x.txt", 5).is_empty());
    }

    #[test]
    fn test_explicit_reference_link() {
        let mut indexer = SemanticIndexer::new();
        let a = indexer
            .index_file(
                "docs/guide.md",
                "For details see /src/main.rs in the codebase.",
                46,
            )
            .unwrap();
        let b = indexer
            .index_file("src/main.rs", "fn main() { println!(\"hello\"); }", 33)
            .unwrap();

        let mut graph = KnowledgeGraph::new();
        graph.add_file(&a, &indexer);
        graph.add_file(&b, &indexer);

        let links = graph.get_links("docs/guide.md");
        // guide.md references /src/main.rs, and b.path is "src/main.rs"
        // The explicit reference check uses ends_with, so it should match
        assert!(links
            .iter()
            .any(|l| l.link_type == LinkType::ExplicitReference));
    }
}
