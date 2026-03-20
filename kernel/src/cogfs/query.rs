//! Natural language query engine over the indexed file corpus.

use std::collections::HashSet;

use chrono::{Duration, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::indexer::SemanticIndexer;

/// Result of a natural language query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// File path of the matching document.
    pub path: String,
    /// Relevance score (higher is better).
    pub relevance_score: f64,
    /// Short snippet showing why this file matched.
    pub snippet: String,
    /// Entities from this file that matched the query.
    pub matched_entities: Vec<String>,
}

/// Natural language query engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NaturalQuery {
    /// Stop words to strip from queries.
    query_stop_words: HashSet<String>,
}

/// Parsed date range for temporal queries.
#[derive(Debug, Clone)]
struct DateRange {
    /// Start of the range (inclusive).
    start: chrono::DateTime<Utc>,
    /// End of the range (inclusive).
    end: chrono::DateTime<Utc>,
}

impl Default for NaturalQuery {
    fn default() -> Self {
        Self::new()
    }
}

impl NaturalQuery {
    /// Create a new query engine.
    pub fn new() -> Self {
        let stop = [
            "what",
            "where",
            "when",
            "how",
            "why",
            "find",
            "show",
            "get",
            "list",
            "give",
            "about",
            "files",
            "file",
            "related",
            "containing",
            "with",
            "the",
            "a",
            "an",
            "is",
            "are",
            "was",
            "were",
            "do",
            "does",
            "did",
            "me",
            "my",
            "all",
            "from",
            "to",
            "in",
            "of",
            "for",
            "and",
            "or",
            "that",
            "which",
            "who",
        ];
        Self {
            query_stop_words: stop.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Execute a natural language query against the indexer.
    pub fn query(&self, question: &str, indexer: &SemanticIndexer) -> Vec<QueryResult> {
        let keywords = self.extract_keywords(question);
        let date_range = Self::parse_date_range(question);

        let mut results: Vec<QueryResult> = Vec::new();

        for (path, indexed) in &indexer.index {
            // Date range filter
            if let Some(ref range) = date_range {
                if indexed.indexed_at < range.start || indexed.indexed_at > range.end {
                    continue;
                }
            }

            let mut score = 0.0_f64;
            let mut matched_keywords = Vec::new();
            let mut matched_entities = Vec::new();

            // TF-IDF score for each keyword
            for kw in &keywords {
                let tfidf = indexer.tf_idf(kw, path);
                if tfidf > 0.0 {
                    score += tfidf;
                    matched_keywords.push(kw.clone());
                }
            }

            // Entity matching bonus
            for entity in &indexed.entities {
                let entity_lower = entity.to_lowercase();
                for kw in &keywords {
                    if entity_lower.contains(kw) {
                        score += 0.5;
                        matched_entities.push(entity.clone());
                    }
                }
            }

            // Topic matching bonus
            for topic in &indexed.topics {
                for kw in &keywords {
                    if topic == kw {
                        score += 0.3;
                    }
                }
            }

            if score > 0.0 {
                let snippet = if matched_keywords.is_empty() {
                    format!("Matched via entities in {path}")
                } else {
                    format!("Matched keywords: {}", matched_keywords.join(", "))
                };

                results.push(QueryResult {
                    path: path.clone(),
                    relevance_score: score,
                    snippet,
                    matched_entities,
                });
            }
        }

        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    /// Extract meaningful keywords from a natural language question.
    fn extract_keywords(&self, question: &str) -> Vec<String> {
        question
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|w| w.to_lowercase())
            .filter(|w| w.len() >= 2 && !self.query_stop_words.contains(w))
            .collect()
    }

    /// Attempt to parse temporal expressions like "last week", "this month", "yesterday".
    fn parse_date_range(question: &str) -> Option<DateRange> {
        let lower = question.to_lowercase();
        let now = Utc::now();

        if lower.contains("yesterday") {
            let start = now - Duration::days(1);
            if let (Some(s), Some(e)) = (
                start.date_naive().and_hms_opt(0, 0, 0),
                start.date_naive().and_hms_opt(23, 59, 59),
            ) {
                return Some(DateRange {
                    start: s.and_utc(),
                    end: e.and_utc(),
                });
            }
        }

        if lower.contains("today") {
            if let Some(s) = now.date_naive().and_hms_opt(0, 0, 0) {
                return Some(DateRange {
                    start: s.and_utc(),
                    end: now,
                });
            }
        }

        if lower.contains("last week") {
            return Some(DateRange {
                start: now - Duration::days(7),
                end: now,
            });
        }

        if lower.contains("last month") || lower.contains("this month") {
            return Some(DateRange {
                start: now - Duration::days(30),
                end: now,
            });
        }

        if lower.contains("last year") || lower.contains("this year") {
            return Some(DateRange {
                start: now - Duration::days(365),
                end: now,
            });
        }

        // Explicit date: "on 2024-06-15"
        if let Ok(date_re) = Regex::new(r"(\d{4}-\d{2}-\d{2})") {
            if let Some(cap) = date_re.captures(&lower) {
                if let Ok(date) = chrono::NaiveDate::parse_from_str(&cap[1], "%Y-%m-%d") {
                    if let (Some(s), Some(e)) = (
                        date.and_hms_opt(0, 0, 0),
                        date.and_hms_opt(23, 59, 59),
                    ) {
                        return Some(DateRange {
                            start: s.and_utc(),
                            end: e.and_utc(),
                        });
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (SemanticIndexer, NaturalQuery) {
        let mut indexer = SemanticIndexer::new();
        indexer
            .index_file(
                "docs/architecture.md",
                "The kernel architecture uses Rust for systems programming. \
                 Agents communicate through capability-based governance. \
                 John Smith designed the fuel system.",
                150,
            )
            .unwrap();
        indexer
            .index_file(
                "src/main.rs",
                "fn main() { let supervisor = Supervisor::new(); \
                 supervisor.start(); println!(\"nexus started\"); }",
                90,
            )
            .unwrap();
        indexer
            .index_file(
                "docs/security.md",
                "Security model uses capability tokens and audit trails. \
                 PII redaction at the gateway boundary. Firewall patterns block threats.",
                130,
            )
            .unwrap();
        (indexer, NaturalQuery::new())
    }

    #[test]
    fn test_basic_query() {
        let (indexer, query_engine) = setup();
        let results = query_engine.query("rust programming", &indexer);
        assert!(!results.is_empty());
        assert_eq!(results[0].path, "docs/architecture.md");
    }

    #[test]
    fn test_query_security() {
        let (indexer, query_engine) = setup();
        let results = query_engine.query("security audit firewall", &indexer);
        assert!(!results.is_empty());
        assert_eq!(results[0].path, "docs/security.md");
    }

    #[test]
    fn test_query_no_results() {
        let (indexer, query_engine) = setup();
        let results = query_engine.query("quantum blockchain metaverse", &indexer);
        assert!(results.is_empty());
    }

    #[test]
    fn test_keyword_extraction() {
        let query_engine = NaturalQuery::new();
        let keywords = query_engine.extract_keywords("What files are related to Rust programming?");
        assert!(keywords.contains(&"rust".to_string()));
        assert!(keywords.contains(&"programming".to_string()));
        assert!(!keywords.contains(&"what".to_string()));
        assert!(!keywords.contains(&"files".to_string()));
    }

    #[test]
    fn test_date_range_parsing() {
        assert!(NaturalQuery::parse_date_range("files from last week").is_some());
        assert!(NaturalQuery::parse_date_range("modified yesterday").is_some());
        assert!(NaturalQuery::parse_date_range("changes today").is_some());
        assert!(NaturalQuery::parse_date_range("on 2024-06-15").is_some());
        assert!(NaturalQuery::parse_date_range("show me rust files").is_none());
    }

    #[test]
    fn test_entity_matching_boost() {
        let (indexer, query_engine) = setup();
        let results = query_engine.query("John Smith", &indexer);
        // Should find architecture.md via entity match
        assert!(!results.is_empty());
        let arch_result = results.iter().find(|r| r.path == "docs/architecture.md");
        assert!(arch_result.is_some());
        assert!(!arch_result.unwrap().matched_entities.is_empty());
    }

    #[test]
    fn test_results_sorted_by_relevance() {
        let (indexer, query_engine) = setup();
        let results = query_engine.query("capability governance", &indexer);
        if results.len() >= 2 {
            assert!(results[0].relevance_score >= results[1].relevance_score);
        }
    }

    #[test]
    fn test_date_range_today_filter() {
        let mut indexer = SemanticIndexer::new();
        // Files indexed right now should match "today"
        indexer
            .index_file("recent.txt", "rust code systems", 17)
            .unwrap();
        let query_engine = NaturalQuery::new();
        let results = query_engine.query("rust files modified today", &indexer);
        assert!(!results.is_empty());
        assert_eq!(results[0].path, "recent.txt");
    }
}
