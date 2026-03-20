//! Semantic indexer — extracts word frequencies, entities, and topics from files.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::CogFsError;

/// Supported file extensions for text extraction.
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "txt", "md", "rs", "py", "js", "ts", "json", "toml", "yaml", "yml", "html", "css",
];

/// Common English stop words excluded from frequency analysis.
const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
    "do", "does", "did", "will", "would", "could", "should", "may", "might", "shall", "can", "to",
    "of", "in", "for", "on", "with", "at", "by", "from", "as", "into", "through", "during",
    "before", "after", "above", "below", "between", "out", "off", "over", "under", "again",
    "further", "then", "once", "and", "but", "or", "nor", "not", "so", "yet", "both", "either",
    "neither", "each", "every", "all", "any", "few", "more", "most", "other", "some", "such", "no",
    "only", "own", "same", "than", "too", "very", "just", "because", "if", "when", "while", "how",
    "what", "which", "who", "whom", "this", "that", "these", "those", "it", "its", "he", "she",
    "they", "them", "his", "her", "their", "my", "your", "our", "me", "him", "us", "we", "i",
    "you",
];

/// An indexed file with extracted semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedFile {
    /// Unique identifier for this index entry.
    pub id: Uuid,
    /// Absolute file path.
    pub path: String,
    /// SHA-256 hash of file content at index time.
    pub content_hash: String,
    /// Extracted named entities (people, identifiers, references).
    pub entities: Vec<String>,
    /// Extracted topics / key phrases.
    pub topics: Vec<String>,
    /// Word frequency map (TF component).
    pub word_frequencies: HashMap<String, u32>,
    /// When the file was indexed.
    pub indexed_at: DateTime<Utc>,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Detected file references (paths mentioned in content).
    pub file_references: Vec<String>,
}

/// The semantic indexer processes file content and produces `IndexedFile` records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticIndexer {
    /// All indexed files keyed by path.
    pub index: HashMap<String, IndexedFile>,
    /// Total document count — used for IDF calculation.
    pub doc_count: u64,
    /// Document frequency: how many documents contain each word.
    pub doc_frequency: HashMap<String, u64>,
}

impl Default for SemanticIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticIndexer {
    /// Create a new empty indexer.
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
            doc_count: 0,
            doc_frequency: HashMap::new(),
        }
    }

    /// Check whether a file extension is supported for indexing.
    pub fn is_supported(path: &str) -> bool {
        path.rsplit('.')
            .next()
            .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext))
            .unwrap_or(false)
    }

    /// Index a file from its path and content.
    ///
    /// Returns the `IndexedFile` on success.
    pub fn index_file(
        &mut self,
        path: &str,
        content: &str,
        size_bytes: u64,
    ) -> Result<IndexedFile, CogFsError> {
        if !Self::is_supported(path) {
            return Err(CogFsError::UnsupportedFileType(path.to_string()));
        }

        let content_hash = Self::hash_content(content);
        let word_frequencies = Self::compute_word_frequencies(content);
        let entities = Self::extract_entities(content);
        let topics = Self::extract_topics(&word_frequencies);
        let file_references = Self::extract_file_references(content);

        // Update document frequency for IDF
        let unique_words: HashSet<&String> = word_frequencies.keys().collect();
        // If re-indexing, remove old document frequency contributions
        if let Some(old) = self.index.get(path) {
            let old_words: HashSet<&String> = old.word_frequencies.keys().collect();
            for w in &old_words {
                if let Some(df) = self.doc_frequency.get_mut(*w) {
                    *df = df.saturating_sub(1);
                }
            }
            // doc_count stays the same on re-index
        } else {
            self.doc_count += 1;
        }
        for w in &unique_words {
            *self.doc_frequency.entry((*w).clone()).or_insert(0) += 1;
        }

        let indexed = IndexedFile {
            id: Uuid::new_v4(),
            path: path.to_string(),
            content_hash,
            entities,
            topics,
            word_frequencies,
            indexed_at: Utc::now(),
            size_bytes,
            file_references,
        };

        self.index.insert(path.to_string(), indexed.clone());
        Ok(indexed)
    }

    /// Remove a file from the index.
    pub fn remove_file(&mut self, path: &str) -> Result<(), CogFsError> {
        if let Some(old) = self.index.remove(path) {
            self.doc_count = self.doc_count.saturating_sub(1);
            for w in old.word_frequencies.keys() {
                if let Some(df) = self.doc_frequency.get_mut(w) {
                    *df = df.saturating_sub(1);
                }
            }
            Ok(())
        } else {
            Err(CogFsError::FileNotIndexed(path.to_string()))
        }
    }

    /// Retrieve the index entry for a given path.
    pub fn get(&self, path: &str) -> Option<&IndexedFile> {
        self.index.get(path)
    }

    /// Compute TF-IDF score for a word in a document.
    pub fn tf_idf(&self, word: &str, path: &str) -> f64 {
        let Some(indexed) = self.index.get(path) else {
            return 0.0;
        };
        let tf = indexed.word_frequencies.get(word).copied().unwrap_or(0) as f64;
        let total_terms: u32 = indexed.word_frequencies.values().sum();
        if total_terms == 0 {
            return 0.0;
        }
        let tf_norm = tf / total_terms as f64;

        let df = self.doc_frequency.get(word).copied().unwrap_or(1) as f64;
        let idf = (self.doc_count as f64 / df).ln() + 1.0;

        tf_norm * idf
    }

    /// SHA-256 hash of content.
    fn hash_content(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Tokenize and count word frequencies, excluding stop words.
    fn compute_word_frequencies(content: &str) -> HashMap<String, u32> {
        let stop: HashSet<&str> = STOP_WORDS.iter().copied().collect();
        let mut freqs: HashMap<String, u32> = HashMap::new();

        for word in content.split(|c: char| !c.is_alphanumeric() && c != '_') {
            let lower = word.to_lowercase();
            if lower.len() < 2 || stop.contains(lower.as_str()) {
                continue;
            }
            *freqs.entry(lower).or_insert(0) += 1;
        }
        freqs
    }

    /// Extract entities: capitalized multi-word names, date patterns, identifiers.
    fn extract_entities(content: &str) -> Vec<String> {
        let mut entities = Vec::new();
        let mut seen = HashSet::new();

        // Capitalized sequences (potential names / proper nouns)
        if let Ok(name_re) = Regex::new(r"\b([A-Z][a-z]+(?:\s+[A-Z][a-z]+)+)\b") {
            for cap in name_re.captures_iter(content) {
                let name = cap[1].to_string();
                if seen.insert(name.clone()) {
                    entities.push(name);
                }
            }
        }

        // ISO dates (YYYY-MM-DD)
        if let Ok(date_re) = Regex::new(r"\b(\d{4}-\d{2}-\d{2})\b") {
            for cap in date_re.captures_iter(content) {
                let d = cap[1].to_string();
                if seen.insert(d.clone()) {
                    entities.push(d);
                }
            }
        }

        // UUIDs
        if let Ok(uuid_re) = Regex::new(
            r"\b([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})\b",
        ) {
            for cap in uuid_re.captures_iter(content) {
                let u = cap[1].to_string();
                if seen.insert(u.clone()) {
                    entities.push(u);
                }
            }
        }

        entities
    }

    /// Extract top topics from word frequencies (top-N by count).
    fn extract_topics(word_frequencies: &HashMap<String, u32>) -> Vec<String> {
        let mut sorted: Vec<(&String, &u32)> = word_frequencies.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        sorted
            .into_iter()
            .take(20)
            .map(|(w, _)| w.clone())
            .collect()
    }

    /// Find file path references inside content.
    fn extract_file_references(content: &str) -> Vec<String> {
        let mut refs = Vec::new();
        let mut seen = HashSet::new();

        if let Ok(path_re) = Regex::new(r#"(?:^|[\s"'`(])(/[\w./-]+\.\w+)"#) {
            for cap in path_re.captures_iter(content) {
                let p = cap[1].to_string();
                if seen.insert(p.clone()) {
                    refs.push(p);
                }
            }
        }

        // Relative paths
        if let Ok(rel_re) =
            Regex::new(r#"(?:^|[\s"'`(])((?:\.\./|\./)?[\w]+(?:/[\w.]+)+\.\w+)"#)
        {
            for cap in rel_re.captures_iter(content) {
                let p = cap[1].to_string();
                if seen.insert(p.clone()) {
                    refs.push(p);
                }
            }
        }

        refs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_extensions() {
        assert!(SemanticIndexer::is_supported("foo.rs"));
        assert!(SemanticIndexer::is_supported("bar.md"));
        assert!(SemanticIndexer::is_supported("baz.json"));
        assert!(!SemanticIndexer::is_supported("image.png"));
        assert!(!SemanticIndexer::is_supported("binary.exe"));
    }

    #[test]
    fn test_index_file() {
        let mut indexer = SemanticIndexer::new();
        let content = "Hello world. Rust is great for systems programming.";
        let result = indexer.index_file("test.rs", content, 51);
        assert!(result.is_ok());
        let indexed = result.unwrap();
        assert_eq!(indexed.path, "test.rs");
        assert!(indexed.word_frequencies.contains_key("rust"));
        assert!(indexed.word_frequencies.contains_key("systems"));
        assert_eq!(indexer.doc_count, 1);
    }

    #[test]
    fn test_unsupported_extension() {
        let mut indexer = SemanticIndexer::new();
        let result = indexer.index_file("image.png", "binary data", 100);
        assert!(matches!(result, Err(CogFsError::UnsupportedFileType(_))));
    }

    #[test]
    fn test_remove_file() {
        let mut indexer = SemanticIndexer::new();
        indexer.index_file("test.rs", "hello world", 11).unwrap();
        assert_eq!(indexer.doc_count, 1);
        indexer.remove_file("test.rs").unwrap();
        assert_eq!(indexer.doc_count, 0);
        assert!(indexer.get("test.rs").is_none());
    }

    #[test]
    fn test_remove_missing_file() {
        let mut indexer = SemanticIndexer::new();
        let result = indexer.remove_file("nonexistent.rs");
        assert!(matches!(result, Err(CogFsError::FileNotIndexed(_))));
    }

    #[test]
    fn test_entity_extraction() {
        let content = "John Smith met Alice Johnson on 2024-06-15. \
                        UUID: 550e8400-e29b-41d4-a716-446655440000";
        let entities = SemanticIndexer::extract_entities(content);
        assert!(entities.iter().any(|e| e == "John Smith"));
        assert!(entities.iter().any(|e| e == "Alice Johnson"));
        assert!(entities.iter().any(|e| e == "2024-06-15"));
        assert!(entities
            .iter()
            .any(|e| e == "550e8400-e29b-41d4-a716-446655440000"));
    }

    #[test]
    fn test_file_reference_extraction() {
        let content = "See /home/user/docs/readme.md and ./src/main.rs for details.";
        let refs = SemanticIndexer::extract_file_references(content);
        assert!(refs.iter().any(|r| r == "/home/user/docs/readme.md"));
        assert!(refs.iter().any(|r| r == "./src/main.rs"));
    }

    #[test]
    fn test_tf_idf() {
        let mut indexer = SemanticIndexer::new();
        indexer
            .index_file("a.txt", "rust rust rust python", 21)
            .unwrap();
        indexer
            .index_file("b.txt", "python python python java", 26)
            .unwrap();

        let rust_a = indexer.tf_idf("rust", "a.txt");
        let rust_b = indexer.tf_idf("rust", "b.txt");
        assert!(rust_a > 0.0);
        assert_eq!(rust_b, 0.0);

        let python_a = indexer.tf_idf("python", "a.txt");
        let python_b = indexer.tf_idf("python", "b.txt");
        // python appears in both docs so IDF is lower, but TF differs
        assert!(python_b > python_a);
    }

    #[test]
    fn test_reindex_file() {
        let mut indexer = SemanticIndexer::new();
        indexer.index_file("a.txt", "hello world", 11).unwrap();
        assert_eq!(indexer.doc_count, 1);
        // Re-index with new content
        indexer.index_file("a.txt", "goodbye world", 13).unwrap();
        assert_eq!(indexer.doc_count, 1); // count stays 1
        let indexed = indexer.get("a.txt").unwrap();
        assert!(indexed.word_frequencies.contains_key("goodbye"));
        assert!(!indexed.word_frequencies.contains_key("hello"));
    }

    #[test]
    fn test_topics_extraction() {
        let mut indexer = SemanticIndexer::new();
        let content = "rust rust rust systems systems programming code";
        let indexed = indexer.index_file("a.rs", content, 47).unwrap();
        assert_eq!(indexed.topics[0], "rust");
        assert!(indexed.topics.contains(&"systems".to_string()));
    }

    #[test]
    fn test_content_hash_deterministic() {
        let hash1 = SemanticIndexer::hash_content("hello");
        let hash2 = SemanticIndexer::hash_content("hello");
        assert_eq!(hash1, hash2);
        let hash3 = SemanticIndexer::hash_content("world");
        assert_ne!(hash1, hash3);
    }
}
