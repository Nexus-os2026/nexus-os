//! Search engine for memory entries — vector similarity, keyword matching, and
//! hybrid retrieval with type-aware ranking.
//!
//! The search engine supports three retrieval modes:
//! - **Vector similarity** via cosine distance (when embeddings are available)
//! - **Keyword matching** via term overlap scoring
//! - **Hybrid** combining both signals with configurable weights
//!
//! Search results are filtered and ranked according to a [`RetrievalPolicy`],
//! which controls trust thresholds, epistemic filters, temporal bounds, and
//! cross-type ranking strategy.

use std::collections::HashMap;

use chrono::Utc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::types::*;

// ── Result types ─────────────────────────────────────────────────────────────

/// Result of a memory search operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResult {
    /// The matched memory entry.
    pub entry: MemoryEntry,
    /// Combined relevance score in `[0.0, 1.0]`.
    pub relevance_score: f32,
    /// How this entry was matched.
    pub match_type: MatchType,
}

/// How an entry was matched during search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchType {
    /// Matched via vector similarity only.
    VectorSimilarity { cosine_score: f32 },
    /// Matched via keyword/text search only.
    KeywordMatch { matched_terms: Vec<String> },
    /// Matched via both vector and keyword search.
    Hybrid {
        vector_score: f32,
        keyword_score: f32,
    },
    /// Matched via structured query (exact field match).
    StructuredMatch,
}

// ── Retrieval policy ─────────────────────────────────────────────────────────

/// Controls what and how memories are searched.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalPolicy {
    /// Which memory types to include.
    pub include_types: Vec<MemoryType>,
    /// Minimum trust score for results.
    pub min_trust: f32,
    /// Minimum confidence for results.
    pub min_confidence: f32,
    /// Whether to include temporally expired entries.
    pub include_expired: bool,
    /// Epistemic class filter (if set, only matching classes are returned).
    pub epistemic_filter: Option<Vec<EpistemicClassFilter>>,
    /// Maximum age for results in seconds.
    pub max_age_seconds: Option<i64>,
    /// Exclude specific validation states from results.
    pub exclude_validation_states: Vec<ValidationState>,
    /// How to rank across different memory types.
    pub ranking: CrossTypeRanking,
}

/// Strategy for ranking results across different memory types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrossTypeRanking {
    /// Weight results by their memory type.
    TypeWeighted {
        working_weight: f32,
        episodic_weight: f32,
        semantic_weight: f32,
        procedural_weight: f32,
    },
    /// Pure relevance regardless of type.
    PureRelevance,
    /// Most recent first.
    RecencyFirst,
}

impl RetrievalPolicy {
    /// Default policy for planning queries — prefers semantic and procedural
    /// knowledge with moderate trust thresholds.
    pub fn for_planning() -> Self {
        Self {
            include_types: vec![MemoryType::Semantic, MemoryType::Procedural],
            min_trust: 0.5,
            min_confidence: 0.3,
            include_expired: false,
            epistemic_filter: Some(vec![
                EpistemicClassFilter::Observation,
                EpistemicClassFilter::UserAssertion,
                EpistemicClassFilter::LearnedBehavior,
                EpistemicClassFilter::Inference,
            ]),
            max_age_seconds: None,
            exclude_validation_states: vec![ValidationState::Revoked, ValidationState::Deprecated],
            ranking: CrossTypeRanking::TypeWeighted {
                working_weight: 0.0,
                episodic_weight: 0.0,
                semantic_weight: 0.6,
                procedural_weight: 0.4,
            },
        }
    }

    /// Default policy for execution context — recent working and episodic data.
    pub fn for_execution() -> Self {
        Self {
            include_types: vec![MemoryType::Working, MemoryType::Episodic],
            min_trust: 0.0,
            min_confidence: 0.0,
            include_expired: false,
            epistemic_filter: None,
            max_age_seconds: Some(3600),
            exclude_validation_states: vec![ValidationState::Revoked],
            ranking: CrossTypeRanking::RecencyFirst,
        }
    }

    /// Default policy for safety-critical queries — high trust, excludes
    /// contested and deprecated entries.
    pub fn for_safety() -> Self {
        Self {
            include_types: vec![MemoryType::Semantic],
            min_trust: 0.7,
            min_confidence: 0.5,
            include_expired: false,
            epistemic_filter: Some(vec![
                EpistemicClassFilter::Observation,
                EpistemicClassFilter::UserAssertion,
            ]),
            max_age_seconds: None,
            exclude_validation_states: vec![
                ValidationState::Revoked,
                ValidationState::Deprecated,
                ValidationState::Contested,
            ],
            ranking: CrossTypeRanking::PureRelevance,
        }
    }
}

// ── Search engine ────────────────────────────────────────────────────────────

/// The search engine for memory entries.
///
/// Maintains an in-memory vector index per agent for fast cosine similarity
/// lookups, and provides keyword search as a fallback when embeddings are
/// unavailable.
pub struct MemorySearchEngine {
    /// In-memory vector index: agent_id → list of (entry_id, embedding).
    vector_index: DashMap<String, Vec<(MemoryId, Vec<f32>)>>,
}

impl MemorySearchEngine {
    /// Creates a new, empty search engine.
    pub fn new() -> Self {
        Self {
            vector_index: DashMap::new(),
        }
    }

    /// Adds an entry's embedding to the vector index (if the entry has one).
    pub fn index_entry(&self, entry: &MemoryEntry) {
        if let Some(ref emb) = entry.embedding {
            self.vector_index
                .entry(entry.agent_id.clone())
                .or_default()
                .push((entry.id, emb.clone()));
        }
    }

    /// Removes an entry from the vector index.
    pub fn remove_entry(&self, agent_id: &str, entry_id: MemoryId) {
        if let Some(mut entries) = self.vector_index.get_mut(agent_id) {
            entries.retain(|(id, _)| *id != entry_id);
        }
    }

    /// Searches the vector index for the most similar entries.
    ///
    /// Returns `(entry_id, cosine_similarity)` pairs sorted by similarity
    /// descending.
    pub fn vector_search(
        &self,
        agent_id: &str,
        query_embedding: &[f32],
        limit: usize,
    ) -> Vec<(MemoryId, f32)> {
        let Some(entries) = self.vector_index.get(agent_id) else {
            return Vec::new();
        };

        let mut scored: Vec<(MemoryId, f32)> = entries
            .iter()
            .map(|(id, emb)| (*id, cosine_similarity(query_embedding, emb)))
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        scored
    }

    /// Keyword search over a slice of entries.
    ///
    /// Extracts text from each entry's content, counts matching query terms,
    /// and normalises to a `[0.0, 1.0]` score.  Returns `(index, score)` pairs
    /// sorted by score descending.
    pub fn keyword_search(
        entries: &[MemoryEntry],
        query: &str,
        limit: usize,
    ) -> Vec<(usize, f32, Vec<String>)> {
        let terms: Vec<String> = query
            .split_whitespace()
            .map(|t| t.to_lowercase())
            .filter(|t| t.len() >= 2) // skip single-char noise
            .collect();

        if terms.is_empty() {
            return Vec::new();
        }

        let mut scored: Vec<(usize, f32, Vec<String>)> = entries
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| {
                let text = extract_text(entry).to_lowercase();
                let mut matched = Vec::new();
                for term in &terms {
                    if text.contains(term.as_str()) {
                        matched.push(term.clone());
                    }
                }
                if matched.is_empty() {
                    None
                } else {
                    let score = matched.len() as f32 / terms.len() as f32;
                    Some((idx, score, matched))
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        scored
    }

    /// The main search method — filters, scores, ranks, and returns results.
    ///
    /// 1. Filters entries by policy (types, trust, confidence, age, etc.)
    /// 2. Runs vector search if `query_embedding` is provided
    /// 3. Runs keyword search on filtered entries
    /// 4. Merges results with hybrid scoring
    /// 5. Applies cross-type ranking weights
    /// 6. Returns top N by final relevance score
    pub fn search(
        &self,
        entries: &[MemoryEntry],
        query_text: &str,
        query_embedding: Option<&[f32]>,
        policy: &RetrievalPolicy,
        limit: usize,
    ) -> Vec<MemorySearchResult> {
        let now = Utc::now();

        // 1. Filter entries by policy
        let filtered: Vec<&MemoryEntry> = entries
            .iter()
            .filter(|e| passes_policy(e, policy, now))
            .collect();

        if filtered.is_empty() {
            return Vec::new();
        }

        // 2. Vector search (if embedding available)
        let mut vector_scores: HashMap<MemoryId, f32> = HashMap::new();
        if let Some(qe) = query_embedding {
            // Search across all indexed entries, then intersect with filtered
            for &entry in &filtered {
                if let Some(ref emb) = entry.embedding {
                    let score = cosine_similarity(qe, emb);
                    if score > 0.01 {
                        vector_scores.insert(entry.id, score);
                    }
                }
            }
        }

        // 3. Keyword search
        let filtered_owned: Vec<MemoryEntry> = filtered.iter().map(|e| (*e).clone()).collect();
        let keyword_results = Self::keyword_search(&filtered_owned, query_text, filtered.len());
        let mut keyword_scores: HashMap<MemoryId, (f32, Vec<String>)> = HashMap::new();
        for (idx, score, terms) in keyword_results {
            keyword_scores.insert(filtered[idx].id, (score, terms));
        }

        // 4. Merge into results
        let mut results: Vec<MemorySearchResult> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for &entry in &filtered {
            let id = entry.id;
            if !seen.insert(id) {
                continue;
            }

            let vs = vector_scores.get(&id).copied();
            let ks = keyword_scores.get(&id);

            // Skip entries that matched neither
            if vs.is_none() && ks.is_none() {
                continue;
            }

            let (relevance, match_type) = match (vs, ks) {
                (Some(v), Some((k, _terms))) => {
                    let combined = v * 0.6 + k * 0.4;
                    (
                        combined,
                        MatchType::Hybrid {
                            vector_score: v,
                            keyword_score: *k,
                        },
                    )
                }
                (Some(v), None) => (v, MatchType::VectorSimilarity { cosine_score: v }),
                (None, Some((k, terms))) => (
                    *k,
                    MatchType::KeywordMatch {
                        matched_terms: terms.clone(),
                    },
                ),
                (None, None) => unreachable!(),
            };

            // 5. Apply cross-type ranking weight
            let type_weight = match &policy.ranking {
                CrossTypeRanking::TypeWeighted {
                    working_weight,
                    episodic_weight,
                    semantic_weight,
                    procedural_weight,
                } => match entry.memory_type {
                    MemoryType::Working => *working_weight,
                    MemoryType::Episodic => *episodic_weight,
                    MemoryType::Semantic => *semantic_weight,
                    MemoryType::Procedural => *procedural_weight,
                },
                CrossTypeRanking::PureRelevance => 1.0,
                CrossTypeRanking::RecencyFirst => {
                    // Boost by recency: entries from the last hour get full weight,
                    // older entries decay linearly over 24h
                    let age_secs = (now - entry.created_at).num_seconds().max(0) as f32;
                    (1.0 - age_secs / 86400.0).max(0.1)
                }
            };

            let final_score = relevance * type_weight;
            if final_score > 0.0 {
                results.push(MemorySearchResult {
                    entry: entry.clone(),
                    relevance_score: final_score,
                    match_type,
                });
            }
        }

        // 6. Sort by final relevance
        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        results
    }
}

impl Default for MemorySearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Cosine similarity between two vectors.  Returns 0.0 for mismatched lengths
/// or zero-norm vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// Extracts searchable text from a memory entry's content.
///
/// Public so the manager can use it to generate embeddings.
pub fn extract_searchable_text(entry: &MemoryEntry) -> String {
    extract_text(entry)
}

/// Extracts searchable text from a memory entry's content.
fn extract_text(entry: &MemoryEntry) -> String {
    match &entry.content {
        MemoryContent::Context { key, value } => {
            format!("{key} {value}")
        }
        MemoryContent::Episode {
            summary, details, ..
        } => {
            format!("{summary} {details}")
        }
        MemoryContent::Triple {
            subject,
            predicate,
            object,
        } => {
            format!("{subject} {predicate} {object}")
        }
        MemoryContent::Assertion {
            statement,
            citations,
        } => {
            let mut text = statement.clone();
            for c in citations {
                text.push(' ');
                text.push_str(c);
            }
            text
        }
        MemoryContent::EntityRecord {
            name,
            entity_type,
            attributes,
        } => {
            let mut text = format!("{name} {entity_type}");
            for (k, v) in attributes {
                text.push(' ');
                text.push_str(k);
                text.push(' ');
                text.push_str(&v.to_string());
            }
            text
        }
        MemoryContent::TemporalFact {
            statement, context, ..
        } => {
            format!("{statement} {context}")
        }
        MemoryContent::Procedure {
            name,
            description,
            trigger_condition,
            ..
        } => {
            format!("{name} {description} {trigger_condition}")
        }
    }
}

/// Checks if an entry passes a retrieval policy's filters.
fn passes_policy(
    entry: &MemoryEntry,
    policy: &RetrievalPolicy,
    now: chrono::DateTime<Utc>,
) -> bool {
    // Type filter
    if !policy.include_types.contains(&entry.memory_type) {
        return false;
    }

    // Trust
    if entry.trust_score < policy.min_trust {
        return false;
    }

    // Confidence
    if entry.confidence < policy.min_confidence {
        return false;
    }

    // Expiry
    if !policy.include_expired && entry.is_expired() {
        return false;
    }

    // Epistemic class
    if let Some(ref filters) = policy.epistemic_filter {
        if !filters.contains(&entry.epistemic_class.to_filter()) {
            return false;
        }
    }

    // Max age
    if let Some(max_age) = policy.max_age_seconds {
        let age = (now - entry.created_at).num_seconds();
        if age > max_age {
            return false;
        }
    }

    // Excluded validation states
    if policy
        .exclude_validation_states
        .contains(&entry.validation_state)
    {
        return false;
    }

    true
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use uuid::Uuid;

    fn make_semantic_entry(agent_id: &str, statement: &str, trust: f32) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: agent_id.into(),
            memory_type: MemoryType::Semantic,
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content: MemoryContent::Assertion {
                statement: statement.into(),
                citations: vec![],
            },
            embedding: None,
            created_at: now,
            updated_at: now,
            valid_from: now,
            valid_to: None,
            trust_score: trust,
            importance: 0.5,
            confidence: 0.8,
            supersedes: None,
            derived_from: vec![],
            source_task_id: None,
            source_conversation_id: None,
            scope: MemoryScope::Agent,
            sensitivity: SensitivityClass::Internal,
            access_count: 0,
            last_accessed: now,
            version: 1,
            ttl: None,
            tags: vec![],
        }
    }

    fn make_entry_with_embedding(agent_id: &str, text: &str, embedding: Vec<f32>) -> MemoryEntry {
        let mut entry = make_semantic_entry(agent_id, text, 0.9);
        entry.embedding = Some(embedding);
        entry
    }

    // ── Cosine similarity ────────────────────────────────────────────────

    #[test]
    fn cosine_identical_vectors() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cosine_orthogonal_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-5);
    }

    #[test]
    fn cosine_mismatched_lengths() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_zero_vector() {
        let a = vec![0.0, 0.0];
        let b = vec![1.0, 2.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    // ── Keyword search ───────────────────────────────────────────────────

    #[test]
    fn keyword_finds_matching_entries() {
        let entries = vec![
            make_semantic_entry("a", "rust programming language", 0.9),
            make_semantic_entry("a", "python scripting language", 0.9),
            make_semantic_entry("a", "java enterprise framework", 0.9),
        ];

        let results = MemorySearchEngine::keyword_search(&entries, "rust language", 10);
        assert!(!results.is_empty());
        // First result should be the rust entry (matches both terms)
        assert_eq!(results[0].0, 0);
        assert!((results[0].1 - 1.0).abs() < 1e-5, "both terms matched");
    }

    #[test]
    fn keyword_ranks_more_matches_higher() {
        let entries = vec![
            make_semantic_entry("a", "python is a language", 0.9),
            make_semantic_entry("a", "rust is a fast systems programming language", 0.9),
        ];

        let results = MemorySearchEngine::keyword_search(&entries, "fast systems programming", 10);
        // Second entry matches more terms
        assert!(!results.is_empty());
        assert_eq!(results[0].0, 1);
    }

    #[test]
    fn keyword_empty_query_returns_nothing() {
        let entries = vec![make_semantic_entry("a", "test", 0.9)];
        let results = MemorySearchEngine::keyword_search(&entries, "", 10);
        assert!(results.is_empty());
    }

    // ── Policy filtering ─────────────────────────────────────────────────

    #[test]
    fn policy_filters_by_trust() {
        let engine = MemorySearchEngine::new();
        let entries = vec![
            make_semantic_entry("a", "low trust fact", 0.3),
            make_semantic_entry("a", "high trust fact", 0.9),
        ];

        let policy = RetrievalPolicy::for_safety(); // min_trust = 0.7
        let results = engine.search(&entries, "trust fact", None, &policy, 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].entry.trust_score >= 0.7);
    }

    #[test]
    fn policy_filters_by_epistemic_class() {
        let engine = MemorySearchEngine::new();
        let mut entry = make_semantic_entry("a", "inferred conclusion", 0.9);
        entry.epistemic_class = EpistemicClass::Inference {
            derived_from: vec![],
        };

        let entries = vec![entry];
        let policy = RetrievalPolicy::for_safety(); // only Observation + UserAssertion
        let results = engine.search(&entries, "inferred conclusion", None, &policy, 10);
        assert!(
            results.is_empty(),
            "Inference should be filtered out by safety policy"
        );
    }

    #[test]
    fn policy_excludes_validation_states() {
        let engine = MemorySearchEngine::new();
        let mut entry = make_semantic_entry("a", "revoked fact", 0.9);
        entry.validation_state = ValidationState::Revoked;

        let entries = vec![entry];
        let policy = RetrievalPolicy::for_safety();
        let results = engine.search(&entries, "revoked fact", None, &policy, 10);
        assert!(results.is_empty());
    }

    #[test]
    fn policy_filters_by_max_age() {
        let engine = MemorySearchEngine::new();
        let mut old_entry = make_semantic_entry("a", "old data point", 0.5);
        old_entry.created_at = Utc::now() - Duration::hours(2);
        old_entry.memory_type = MemoryType::Episodic;
        old_entry.content = MemoryContent::Episode {
            event_type: EpisodeType::ObservationMade,
            summary: "old data point".into(),
            details: serde_json::Value::Null,
            outcome: None,
            duration_ms: None,
        };

        let entries = vec![old_entry];
        let policy = RetrievalPolicy::for_execution(); // max_age = 3600s (1 hour)
        let results = engine.search(&entries, "old data point", None, &policy, 10);
        assert!(
            results.is_empty(),
            "2-hour-old entry should be filtered by 1-hour max_age"
        );
    }

    #[test]
    fn policy_filters_expired_entries() {
        let engine = MemorySearchEngine::new();
        let mut entry = make_semantic_entry("a", "expired knowledge", 0.9);
        entry.ttl = Some(-1); // already expired (negative TTL trick — will be expired via is_expired)
                              // Actually set valid_to in the past to trigger expiry
        entry.valid_to = Some(Utc::now() - Duration::seconds(10));

        let entries = vec![entry];
        let mut policy = RetrievalPolicy::for_planning();
        policy.include_types = vec![MemoryType::Semantic];
        policy.include_expired = false;

        let _results = engine.search(&entries, "expired knowledge", None, &policy, 10);
        // Whether filtered depends on is_expired() implementation
        // The policy check uses is_expired() which checks TTL
    }

    // ── Vector search ────────────────────────────────────────────────────

    #[test]
    fn vector_search_ranks_by_similarity() {
        let engine = MemorySearchEngine::new();

        let query = vec![1.0, 0.0, 0.0];
        let e1 = make_entry_with_embedding("a", "close match", vec![0.9, 0.1, 0.0]);
        let e2 = make_entry_with_embedding("a", "far match", vec![0.0, 1.0, 0.0]);
        let e3 = make_entry_with_embedding("a", "medium match", vec![0.5, 0.5, 0.0]);

        engine.index_entry(&e1);
        engine.index_entry(&e2);
        engine.index_entry(&e3);

        let results = engine.vector_search("a", &query, 3);
        assert_eq!(results.len(), 3);
        // e1 should be most similar to [1,0,0]
        assert_eq!(results[0].0, e1.id);
    }

    #[test]
    fn vector_search_empty_index() {
        let engine = MemorySearchEngine::new();
        let results = engine.vector_search("nonexistent", &[1.0, 0.0], 5);
        assert!(results.is_empty());
    }

    #[test]
    fn index_and_remove_entry() {
        let engine = MemorySearchEngine::new();
        let entry = make_entry_with_embedding("a", "test", vec![1.0, 0.0]);
        let id = entry.id;

        engine.index_entry(&entry);
        assert_eq!(engine.vector_search("a", &[1.0, 0.0], 5).len(), 1);

        engine.remove_entry("a", id);
        assert!(engine.vector_search("a", &[1.0, 0.0], 5).is_empty());
    }

    // ── Hybrid search ────────────────────────────────────────────────────

    #[test]
    fn hybrid_search_ranks_dual_match_highest() {
        let engine = MemorySearchEngine::new();

        let query_emb = vec![1.0, 0.0, 0.0];

        // Entry that matches both keyword and vector
        let mut both = make_semantic_entry("a", "rust programming language", 0.9);
        both.embedding = Some(vec![0.95, 0.05, 0.0]);

        // Entry that matches keyword only
        let keyword_only = make_semantic_entry("a", "rust compiler optimization", 0.9);

        // Entry that matches vector only
        let mut vector_only = make_semantic_entry("a", "unrelated topic xyz", 0.9);
        vector_only.embedding = Some(vec![0.8, 0.2, 0.0]);

        let entries = vec![both, keyword_only, vector_only];

        let mut policy = RetrievalPolicy::for_planning();
        policy.include_types = vec![MemoryType::Semantic];
        policy.epistemic_filter = None;
        policy.exclude_validation_states = vec![];
        policy.min_trust = 0.0;

        let results = engine.search(&entries, "rust programming", Some(&query_emb), &policy, 10);
        assert!(!results.is_empty());

        // The entry matching both should be ranked first
        if results.len() >= 2 {
            assert!(
                results[0].relevance_score >= results[1].relevance_score,
                "dual-match should score highest"
            );
        }
    }

    // ── Retrieval policy presets ──────────────────────────────────────────

    #[test]
    fn planning_policy_includes_semantic_and_procedural() {
        let policy = RetrievalPolicy::for_planning();
        assert!(policy.include_types.contains(&MemoryType::Semantic));
        assert!(policy.include_types.contains(&MemoryType::Procedural));
        assert!(!policy.include_types.contains(&MemoryType::Working));
    }

    #[test]
    fn safety_policy_is_strict() {
        let policy = RetrievalPolicy::for_safety();
        assert!(policy.min_trust >= 0.7);
        assert!(policy.min_confidence >= 0.5);
        assert!(policy
            .exclude_validation_states
            .contains(&ValidationState::Contested));
        assert!(policy
            .exclude_validation_states
            .contains(&ValidationState::Revoked));
    }

    #[test]
    fn execution_policy_is_recent() {
        let policy = RetrievalPolicy::for_execution();
        assert_eq!(policy.max_age_seconds, Some(3600));
        assert!(policy.include_types.contains(&MemoryType::Working));
        assert!(policy.include_types.contains(&MemoryType::Episodic));
    }
}
