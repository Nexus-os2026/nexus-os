//! Context builder — assembles rich context packages from the cognitive filesystem
//! for agent system prompts.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::graph::KnowledgeGraph;
use super::indexer::SemanticIndexer;
use super::query::NaturalQuery;
use super::watcher::FileWatcher;

/// A rich context package assembled from indexed files for a given topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPackage {
    /// The topic this context was built for.
    pub topic: String,
    /// Files most relevant to the topic, with relevance scores.
    pub relevant_files: Vec<ContextFile>,
    /// Aggregated summary of the topic across files.
    pub summary: String,
    /// All entities related to the topic.
    pub entities: Vec<String>,
    /// Files that changed recently and relate to the topic.
    pub recent_changes: Vec<String>,
    /// When this context was built.
    pub built_at: DateTime<Utc>,
}

/// A file included in a context package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFile {
    /// File path.
    pub path: String,
    /// Relevance to the topic.
    pub relevance: f64,
    /// Top topics from this file.
    pub topics: Vec<String>,
}

/// Builds context packages by combining indexer, graph, query, and watcher data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBuilder {
    /// Maximum number of files to include in a context package.
    pub max_files: usize,
    /// Maximum number of entities to include.
    pub max_entities: usize,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextBuilder {
    /// Create a new context builder with default limits.
    pub fn new() -> Self {
        Self {
            max_files: 10,
            max_entities: 30,
        }
    }

    /// Create a context builder with custom limits.
    pub fn with_limits(max_files: usize, max_entities: usize) -> Self {
        Self {
            max_files,
            max_entities,
        }
    }

    /// Build a context package for a given topic.
    ///
    /// Combines:
    /// - Query results for the topic
    /// - Knowledge graph links from top results
    /// - Entities from matching files
    /// - Dirty files from the watcher as recent changes
    pub fn build_context(
        &self,
        topic: &str,
        indexer: &SemanticIndexer,
        graph: &KnowledgeGraph,
        watcher: &FileWatcher,
    ) -> ContextPackage {
        let query_engine = NaturalQuery::new();
        let query_results = query_engine.query(topic, indexer);

        // Collect relevant files from query results
        let mut relevant_files: Vec<ContextFile> = Vec::new();
        let mut seen_paths: HashSet<String> = HashSet::new();

        for qr in &query_results {
            if seen_paths.insert(qr.path.clone()) {
                if let Some(indexed) = indexer.get(&qr.path) {
                    relevant_files.push(ContextFile {
                        path: qr.path.clone(),
                        relevance: qr.relevance_score,
                        topics: indexed.topics.clone(),
                    });
                }
            }
            if relevant_files.len() >= self.max_files {
                break;
            }
        }

        // Expand via knowledge graph — add related files from top results
        let top_paths: Vec<String> = relevant_files
            .iter()
            .take(3)
            .map(|f| f.path.clone())
            .collect();

        for path in &top_paths {
            let related = graph.find_related(path, 3);
            for (rel_path, strength) in related {
                if seen_paths.insert(rel_path.clone()) && relevant_files.len() < self.max_files {
                    if let Some(indexed) = indexer.get(&rel_path) {
                        relevant_files.push(ContextFile {
                            path: rel_path,
                            relevance: strength * 0.5, // downweight graph-only results
                            topics: indexed.topics.clone(),
                        });
                    }
                }
            }
        }

        // Sort by relevance
        relevant_files.sort_by(|a, b| {
            b.relevance
                .partial_cmp(&a.relevance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        relevant_files.truncate(self.max_files);

        // Collect entities
        let mut entities: Vec<String> = Vec::new();
        let mut entity_set: HashSet<String> = HashSet::new();
        for cf in &relevant_files {
            if let Some(indexed) = indexer.get(&cf.path) {
                for entity in &indexed.entities {
                    if entity_set.insert(entity.clone()) && entities.len() < self.max_entities {
                        entities.push(entity.clone());
                    }
                }
            }
        }

        // Recent changes from watcher
        let recent_changes: Vec<String> = watcher
            .dirty_files()
            .iter()
            .filter(|t| seen_paths.contains(&t.path))
            .map(|t| t.path.clone())
            .collect();

        // Build summary
        let summary = self.build_summary(topic, &relevant_files, &entities);

        ContextPackage {
            topic: topic.to_string(),
            relevant_files,
            summary,
            entities,
            recent_changes,
            built_at: Utc::now(),
        }
    }

    /// Build a human-readable summary of the context.
    fn build_summary(&self, topic: &str, files: &[ContextFile], entities: &[String]) -> String {
        let file_count = files.len();
        let entity_count = entities.len();

        let top_topics: Vec<String> = {
            let mut all_topics: Vec<&String> = files.iter().flat_map(|f| f.topics.iter()).collect();
            let mut seen = HashSet::new();
            all_topics.retain(|t| seen.insert(*t));
            all_topics.into_iter().take(5).cloned().collect()
        };

        let mut parts = vec![format!(
            "Context for \"{topic}\": {file_count} relevant file(s) found"
        )];

        if !top_topics.is_empty() {
            parts.push(format!("Key themes: {}", top_topics.join(", ")));
        }

        if entity_count > 0 {
            parts.push(format!("{entity_count} related entity/entities identified"));
        }

        parts.join(". ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (SemanticIndexer, KnowledgeGraph, FileWatcher) {
        let mut indexer = SemanticIndexer::new();
        indexer
            .index_file(
                "docs/architecture.md",
                "The kernel architecture uses Rust for systems programming. \
                 Agents communicate through governed capabilities. \
                 John Smith designed the fuel system on 2024-06-15.",
                160,
            )
            .unwrap();
        indexer
            .index_file(
                "src/kernel.rs",
                "pub struct Kernel { fuel: FuelLedger, audit: AuditTrail } \
                 impl Kernel { fn new() -> Self { todo!() } }",
                100,
            )
            .unwrap();
        indexer
            .index_file(
                "docs/security.md",
                "Security model: capability tokens, PII redaction, audit trails. \
                 Firewall patterns block threats. Alice Johnson reviewed.",
                120,
            )
            .unwrap();

        let mut graph = KnowledgeGraph::new();
        for indexed in indexer.index.values() {
            graph.add_file(indexed, &indexer);
        }

        let mut watcher = FileWatcher::default();
        watcher.report_file("docs/architecture.md", Utc::now(), "hash1");
        // Leave it dirty to simulate a recent change

        (indexer, graph, watcher)
    }

    #[test]
    fn test_build_context_basic() {
        let (indexer, graph, watcher) = setup();
        let builder = ContextBuilder::new();
        let ctx = builder.build_context("kernel architecture", &indexer, &graph, &watcher);

        assert_eq!(ctx.topic, "kernel architecture");
        assert!(!ctx.relevant_files.is_empty());
        assert!(!ctx.summary.is_empty());
        // architecture.md should be in the results
        assert!(ctx
            .relevant_files
            .iter()
            .any(|f| f.path == "docs/architecture.md"));
    }

    #[test]
    fn test_context_includes_entities() {
        let (indexer, graph, watcher) = setup();
        let builder = ContextBuilder::new();
        let ctx = builder.build_context("kernel architecture", &indexer, &graph, &watcher);

        // Should include John Smith from architecture.md
        assert!(ctx.entities.iter().any(|e| e.contains("John Smith")));
    }

    #[test]
    fn test_context_recent_changes() {
        let (indexer, graph, watcher) = setup();
        let builder = ContextBuilder::new();
        let ctx = builder.build_context("kernel architecture", &indexer, &graph, &watcher);

        // architecture.md is dirty in watcher and in query results
        assert!(ctx
            .recent_changes
            .contains(&"docs/architecture.md".to_string()));
    }

    #[test]
    fn test_context_with_limits() {
        let (indexer, graph, watcher) = setup();
        let builder = ContextBuilder::with_limits(1, 2);
        let ctx = builder.build_context("kernel", &indexer, &graph, &watcher);

        assert!(ctx.relevant_files.len() <= 1);
        assert!(ctx.entities.len() <= 2);
    }

    #[test]
    fn test_context_no_results() {
        let (indexer, graph, watcher) = setup();
        let builder = ContextBuilder::new();
        let ctx = builder.build_context("quantum blockchain", &indexer, &graph, &watcher);

        assert!(ctx.relevant_files.is_empty());
        assert!(ctx.entities.is_empty());
    }

    #[test]
    fn test_summary_format() {
        let (indexer, graph, watcher) = setup();
        let builder = ContextBuilder::new();
        let ctx = builder.build_context("security audit", &indexer, &graph, &watcher);

        assert!(ctx.summary.contains("Context for \"security audit\""));
        assert!(ctx.summary.contains("relevant file(s) found"));
    }

    #[test]
    fn test_graph_expansion() {
        let (indexer, graph, watcher) = setup();
        let builder = ContextBuilder::new();
        let ctx = builder.build_context("rust systems", &indexer, &graph, &watcher);

        // Should include files found via graph links, not just direct query hits
        let paths: Vec<&str> = ctx.relevant_files.iter().map(|f| f.path.as_str()).collect();
        assert!(!paths.is_empty());
    }
}
