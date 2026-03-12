//! Neural Bridge — governed screen/audio semantic indexer.
//!
//! Creates a searchable database of context entries (screen captures, audio
//! transcripts, clipboard, documents) with PII redaction.  All data stays
//! local and never leaves the device.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single indexed context entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEntry {
    pub id: String,
    pub source: ContextSource,
    /// Extracted (and possibly redacted) text content.
    pub content: String,
    /// Short summary of the content.
    pub summary: String,
    pub timestamp: u64,
    pub tags: Vec<String>,
    pub pii_redacted: bool,
}

/// Where a context entry originated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextSource {
    Screen {
        app_name: String,
        window_title: String,
    },
    Audio {
        duration_secs: f32,
    },
    Clipboard,
    Document {
        path: String,
    },
    UserInput {
        source: String,
    },
}

/// Parameters for searching the context store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextQuery {
    pub query: String,
    /// Optional `(start, end)` timestamp window.
    pub time_range: Option<(u64, u64)>,
    /// Filter by source type name (e.g. `"Screen"`, `"Audio"`).
    pub source_filter: Option<Vec<String>>,
    pub max_results: usize,
}

/// A search hit with its relevance score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextResult {
    pub entry: ContextEntry,
    pub relevance_score: f32,
}

/// Runtime configuration for the Neural Bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralBridgeConfig {
    pub enabled: bool,
    pub capture_interval_secs: u64,
    pub max_entries: usize,
    pub pii_redaction_enabled: bool,
    pub auto_summarize: bool,
    pub retention_days: u32,
}

impl Default for NeuralBridgeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            capture_interval_secs: 30,
            max_entries: 10_000,
            pii_redaction_enabled: true,
            auto_summarize: true,
            retention_days: 30,
        }
    }
}

/// Aggregate statistics about the Neural Bridge store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralBridgeStats {
    pub total_entries: usize,
    pub entries_by_source: HashMap<String, usize>,
    pub oldest_entry: Option<u64>,
    pub newest_entry: Option<u64>,
    pub total_pii_redacted: usize,
    pub storage_estimate_bytes: u64,
}

// ---------------------------------------------------------------------------
// Neural Bridge
// ---------------------------------------------------------------------------

/// Governed semantic indexer for screen, audio, clipboard, and document context.
pub struct NeuralBridge {
    config: NeuralBridgeConfig,
    entries: Vec<ContextEntry>,
    embeddings: Vec<(String, Vec<f32>)>,
    stats: NeuralBridgeStats,
}

impl NeuralBridge {
    pub fn new(config: NeuralBridgeConfig) -> Self {
        Self {
            config,
            entries: Vec::new(),
            embeddings: Vec::new(),
            stats: NeuralBridgeStats {
                total_entries: 0,
                entries_by_source: HashMap::new(),
                oldest_entry: None,
                newest_entry: None,
                total_pii_redacted: 0,
                storage_estimate_bytes: 0,
            },
        }
    }

    // -- public API ---------------------------------------------------------

    /// Ingest raw content from the given source, optionally redacting PII.
    pub fn ingest(
        &mut self,
        source: ContextSource,
        raw_content: &str,
    ) -> Result<ContextEntry, String> {
        let (content, was_redacted) = if self.config.pii_redaction_enabled {
            let redacted = redact_pii(raw_content);
            let changed = redacted != raw_content;
            (redacted, changed)
        } else {
            (raw_content.to_string(), false)
        };

        let summary = if self.config.auto_summarize {
            summarize(&content)
        } else {
            String::new()
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let entry = ContextEntry {
            id: Uuid::new_v4().to_string(),
            source,
            content,
            summary,
            timestamp: now,
            tags: Vec::new(),
            pii_redacted: was_redacted,
        };

        // Evict oldest entries if we're at capacity.
        while self.entries.len() >= self.config.max_entries {
            let removed = self.entries.remove(0);
            self.embeddings.retain(|(id, _)| *id != removed.id);
        }

        self.entries.push(entry.clone());
        self.refresh_stats();

        Ok(entry)
    }

    /// Text-based search over content, summaries, and tags.
    pub fn search(&self, query: &ContextQuery) -> Vec<ContextResult> {
        let query_lower = query.query.to_lowercase();

        let mut results: Vec<ContextResult> = self
            .entries
            .iter()
            .filter(|e| {
                // Time-range filter
                if let Some((start, end)) = query.time_range {
                    if e.timestamp < start || e.timestamp > end {
                        return false;
                    }
                }
                // Source-type filter
                if let Some(ref filters) = query.source_filter {
                    let source_name = source_type_name(&e.source);
                    if !filters.iter().any(|f| f.eq_ignore_ascii_case(&source_name)) {
                        return false;
                    }
                }
                true
            })
            .filter_map(|e| {
                let score = relevance_score(e, &query_lower);
                if score > 0.0 {
                    Some(ContextResult {
                        entry: e.clone(),
                        relevance_score: score,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Sort descending by score.
        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(query.max_results);
        results
    }

    /// Retrieve an entry by ID.
    pub fn get_entry(&self, id: &str) -> Option<&ContextEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Delete an entry by ID. Returns `true` if found and removed.
    pub fn delete_entry(&mut self, id: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.embeddings.retain(|(eid, _)| eid != id);
        let removed = self.entries.len() < before;
        if removed {
            self.refresh_stats();
        }
        removed
    }

    /// Remove all entries with timestamps strictly before `timestamp`.
    /// Returns the number of entries removed.
    pub fn clear_before(&mut self, timestamp: u64) -> usize {
        let before = self.entries.len();
        let removed_ids: Vec<String> = self
            .entries
            .iter()
            .filter(|e| e.timestamp < timestamp)
            .map(|e| e.id.clone())
            .collect();
        self.entries.retain(|e| e.timestamp >= timestamp);
        for rid in &removed_ids {
            self.embeddings.retain(|(id, _)| id != rid);
        }
        let count = before - self.entries.len();
        if count > 0 {
            self.refresh_stats();
        }
        count
    }

    /// Current statistics snapshot.
    pub fn get_stats(&self) -> NeuralBridgeStats {
        self.stats.clone()
    }

    /// Export entries, optionally filtered by a time range.
    pub fn export_entries(&self, time_range: Option<(u64, u64)>) -> Vec<&ContextEntry> {
        self.entries
            .iter()
            .filter(|e| match time_range {
                Some((start, end)) => e.timestamp >= start && e.timestamp <= end,
                None => true,
            })
            .collect()
    }

    /// Add tags to an existing entry.
    pub fn tag_entry(&mut self, id: &str, tags: Vec<String>) -> Result<(), String> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| format!("entry not found: {id}"))?;
        for tag in tags {
            if !entry.tags.contains(&tag) {
                entry.tags.push(tag);
            }
        }
        Ok(())
    }

    /// Read-only access to current config.
    pub fn config(&self) -> &NeuralBridgeConfig {
        &self.config
    }

    /// Toggle the enabled flag.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    // -- internals ----------------------------------------------------------

    fn refresh_stats(&mut self) {
        let mut by_source: HashMap<String, usize> = HashMap::new();
        let mut oldest: Option<u64> = None;
        let mut newest: Option<u64> = None;
        let mut pii_count: usize = 0;
        let mut bytes: u64 = 0;

        for e in &self.entries {
            *by_source.entry(source_type_name(&e.source)).or_insert(0) += 1;
            oldest = Some(oldest.map_or(e.timestamp, |o: u64| o.min(e.timestamp)));
            newest = Some(newest.map_or(e.timestamp, |n: u64| n.max(e.timestamp)));
            if e.pii_redacted {
                pii_count += 1;
            }
            bytes += (e.content.len() + e.summary.len()) as u64;
        }

        self.stats = NeuralBridgeStats {
            total_entries: self.entries.len(),
            entries_by_source: by_source,
            oldest_entry: oldest,
            newest_entry: newest,
            total_pii_redacted: pii_count,
            storage_estimate_bytes: bytes,
        };
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return a human-readable name for a [`ContextSource`] variant.
fn source_type_name(source: &ContextSource) -> String {
    match source {
        ContextSource::Screen { .. } => "Screen".to_string(),
        ContextSource::Audio { .. } => "Audio".to_string(),
        ContextSource::Clipboard => "Clipboard".to_string(),
        ContextSource::Document { .. } => "Document".to_string(),
        ContextSource::UserInput { .. } => "UserInput".to_string(),
    }
}

/// Simple PII redaction using regex-like patterns (no external crate).
fn redact_pii(text: &str) -> String {
    let mut out = text.to_string();

    // Email: simple pattern  word@word.word
    out = redact_pattern(&out, |s| {
        let mut result = String::new();
        let chars: Vec<char> = s.chars().collect();
        let len = chars.len();
        let mut i = 0;
        while i < len {
            if chars[i] == '@' && i > 0 && i + 1 < len {
                // Find start of local part
                let start = {
                    let mut j = i - 1;
                    while j > 0
                        && (chars[j].is_alphanumeric()
                            || chars[j] == '.'
                            || chars[j] == '_'
                            || chars[j] == '-')
                    {
                        j -= 1;
                    }
                    if !(chars[j].is_alphanumeric()
                        || chars[j] == '.'
                        || chars[j] == '_'
                        || chars[j] == '-')
                    {
                        j + 1
                    } else {
                        j
                    }
                };
                // Find end of domain
                let mut end = i + 1;
                let mut has_dot = false;
                while end < len
                    && (chars[end].is_alphanumeric() || chars[end] == '.' || chars[end] == '-')
                {
                    if chars[end] == '.' {
                        has_dot = true;
                    }
                    end += 1;
                }
                if has_dot && start < i {
                    // Replace the email
                    result.truncate(result.len() - (i - start));
                    result.push_str("[EMAIL_REDACTED]");
                    i = end;
                    continue;
                }
            }
            result.push(chars[i]);
            i += 1;
        }
        result
    });

    // Phone numbers: sequences of digits (optionally separated by - or space)
    // that look like phone numbers (10+ digits).
    out = redact_phones(&out);

    // SSN pattern: ###-##-####
    out = redact_ssn(&out);

    out
}

/// Redact phone-number-like patterns (10+ digits, with optional separators).
fn redact_phones(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut result = String::new();
    let mut i = 0;

    while i < len {
        if chars[i].is_ascii_digit()
            || (chars[i] == '+' && i + 1 < len && chars[i + 1].is_ascii_digit())
        {
            let start = i;
            let mut digit_count = 0u32;
            let mut j = i;
            while j < len
                && (chars[j].is_ascii_digit()
                    || chars[j] == '-'
                    || chars[j] == ' '
                    || chars[j] == '('
                    || chars[j] == ')'
                    || chars[j] == '+'
                    || chars[j] == '.')
            {
                if chars[j].is_ascii_digit() {
                    digit_count += 1;
                }
                j += 1;
            }
            if digit_count >= 10 {
                result.push_str("[PHONE_REDACTED]");
                i = j;
                continue;
            }
            // Not a phone — emit chars as-is
            for ch in &chars[start..j] {
                result.push(*ch);
            }
            i = j;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

/// Redact SSN pattern: `###-##-####`
fn redact_ssn(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut result = String::new();
    let mut i = 0;

    while i < len {
        // Check for ###-##-####
        if i + 10 < len
            && chars[i].is_ascii_digit()
            && chars[i + 1].is_ascii_digit()
            && chars[i + 2].is_ascii_digit()
            && chars[i + 3] == '-'
            && chars[i + 4].is_ascii_digit()
            && chars[i + 5].is_ascii_digit()
            && chars[i + 6] == '-'
            && chars[i + 7].is_ascii_digit()
            && chars[i + 8].is_ascii_digit()
            && chars[i + 9].is_ascii_digit()
            && chars[i + 10].is_ascii_digit()
        {
            // Make sure it's not part of a longer digit sequence
            let before_ok = i == 0 || !chars[i - 1].is_ascii_digit();
            let after_ok = i + 11 >= len || !chars[i + 11].is_ascii_digit();
            if before_ok && after_ok {
                result.push_str("[SSN_REDACTED]");
                i += 11;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

/// Apply a transformation function for redaction.
fn redact_pattern(text: &str, f: impl Fn(&str) -> String) -> String {
    f(text)
}

/// Generate a simple summary: first 100 chars + ellipsis.
fn summarize(content: &str) -> String {
    if content.len() <= 100 {
        content.to_string()
    } else {
        let boundary = content
            .char_indices()
            .nth(100)
            .map(|(i, _)| i)
            .unwrap_or(content.len());
        format!("{}...", &content[..boundary])
    }
}

/// Compute a simple text-based relevance score.
fn relevance_score(entry: &ContextEntry, query_lower: &str) -> f32 {
    let content_lower = entry.content.to_lowercase();
    let summary_lower = entry.summary.to_lowercase();

    let mut score: f32 = 0.0;

    // Exact content match
    if content_lower.contains(query_lower) {
        score += 1.0;
    }

    // Exact summary match
    if summary_lower.contains(query_lower) {
        score += 0.8;
    }

    // Tag match
    for tag in &entry.tags {
        if tag.to_lowercase().contains(query_lower) {
            score += 0.5;
        }
    }

    // Partial: check individual words
    if score == 0.0 {
        let words: Vec<&str> = query_lower.split_whitespace().collect();
        let mut word_hits = 0u32;
        for w in &words {
            if content_lower.contains(w) || summary_lower.contains(w) {
                word_hits += 1;
            }
        }
        if !words.is_empty() && word_hits > 0 {
            score += 0.3 * (word_hits as f32 / words.len() as f32);
        }
    }

    score
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_bridge() -> NeuralBridge {
        NeuralBridge::new(NeuralBridgeConfig::default())
    }

    #[test]
    fn test_config_defaults() {
        let cfg = NeuralBridgeConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.capture_interval_secs, 30);
        assert_eq!(cfg.max_entries, 10_000);
        assert!(cfg.pii_redaction_enabled);
        assert!(cfg.auto_summarize);
        assert_eq!(cfg.retention_days, 30);
    }

    #[test]
    fn test_ingest_creates_entry() {
        let mut bridge = default_bridge();
        let entry = bridge
            .ingest(ContextSource::Clipboard, "hello world")
            .unwrap();
        assert_eq!(entry.content, "hello world");
        assert!(!entry.id.is_empty());
        assert!(entry.timestamp > 0);
        assert_eq!(bridge.get_stats().total_entries, 1);
    }

    #[test]
    fn test_ingest_redacts_email() {
        let mut bridge = default_bridge();
        let entry = bridge
            .ingest(ContextSource::Clipboard, "contact user@test.com for info")
            .unwrap();
        assert!(entry.pii_redacted);
        assert!(entry.content.contains("[EMAIL_REDACTED]"));
        assert!(!entry.content.contains("user@test.com"));
    }

    #[test]
    fn test_ingest_redacts_phone() {
        let mut bridge = default_bridge();
        let entry = bridge
            .ingest(ContextSource::Clipboard, "call 555-123-4567 now")
            .unwrap();
        assert!(entry.pii_redacted);
        assert!(entry.content.contains("[PHONE_REDACTED]"));
        assert!(!entry.content.contains("555-123-4567"));
    }

    #[test]
    fn test_ingest_redacts_ssn() {
        let mut bridge = default_bridge();
        let entry = bridge
            .ingest(ContextSource::Clipboard, "SSN is 123-45-6789 here")
            .unwrap();
        assert!(entry.pii_redacted);
        assert!(entry.content.contains("[SSN_REDACTED]"));
        assert!(!entry.content.contains("123-45-6789"));
    }

    #[test]
    fn test_search_by_content() {
        let mut bridge = default_bridge();
        bridge
            .ingest(ContextSource::Clipboard, "rust programming language")
            .unwrap();
        bridge
            .ingest(ContextSource::Clipboard, "python scripting")
            .unwrap();

        let results = bridge.search(&ContextQuery {
            query: "rust".to_string(),
            time_range: None,
            source_filter: None,
            max_results: 10,
        });
        assert_eq!(results.len(), 1);
        assert!(results[0].entry.content.contains("rust"));
    }

    #[test]
    fn test_search_by_time_range() {
        let mut bridge = default_bridge();
        let e1 = bridge
            .ingest(ContextSource::Clipboard, "early entry")
            .unwrap();
        let ts = e1.timestamp;

        let results = bridge.search(&ContextQuery {
            query: "entry".to_string(),
            time_range: Some((ts, ts + 1)),
            source_filter: None,
            max_results: 10,
        });
        assert!(!results.is_empty());

        // Out-of-range
        let results2 = bridge.search(&ContextQuery {
            query: "entry".to_string(),
            time_range: Some((ts + 100, ts + 200)),
            source_filter: None,
            max_results: 10,
        });
        assert!(results2.is_empty());
    }

    #[test]
    fn test_search_by_source_filter() {
        let mut bridge = default_bridge();
        bridge
            .ingest(ContextSource::Clipboard, "clipboard data")
            .unwrap();
        bridge
            .ingest(ContextSource::Audio { duration_secs: 5.0 }, "audio data")
            .unwrap();

        let results = bridge.search(&ContextQuery {
            query: "data".to_string(),
            time_range: None,
            source_filter: Some(vec!["Audio".to_string()]),
            max_results: 10,
        });
        assert_eq!(results.len(), 1);
        assert!(results[0].entry.content.contains("audio"));
    }

    #[test]
    fn test_search_max_results() {
        let mut bridge = default_bridge();
        for i in 0..10 {
            bridge
                .ingest(ContextSource::Clipboard, &format!("item number {i}"))
                .unwrap();
        }

        let results = bridge.search(&ContextQuery {
            query: "item".to_string(),
            time_range: None,
            source_filter: None,
            max_results: 3,
        });
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_eviction_at_max_entries() {
        let mut bridge = NeuralBridge::new(NeuralBridgeConfig {
            max_entries: 3,
            ..NeuralBridgeConfig::default()
        });

        for i in 0..5 {
            bridge
                .ingest(ContextSource::Clipboard, &format!("entry {i}"))
                .unwrap();
        }
        assert_eq!(bridge.get_stats().total_entries, 3);
    }

    #[test]
    fn test_delete_entry() {
        let mut bridge = default_bridge();
        let entry = bridge
            .ingest(ContextSource::Clipboard, "delete me")
            .unwrap();
        assert!(bridge.delete_entry(&entry.id));
        assert!(bridge.get_entry(&entry.id).is_none());
        assert_eq!(bridge.get_stats().total_entries, 0);
        // Double-delete returns false
        assert!(!bridge.delete_entry(&entry.id));
    }

    #[test]
    fn test_clear_before_timestamp() {
        let mut bridge = default_bridge();
        bridge
            .ingest(ContextSource::Clipboard, "old entry")
            .unwrap();
        bridge
            .ingest(ContextSource::Clipboard, "new entry")
            .unwrap();

        // Both entries share the same timestamp (same second).
        // Manually backdate the first entry so clear_before can distinguish them.
        bridge.entries[0].timestamp -= 10;
        bridge.refresh_stats();

        let boundary = bridge.entries[0].timestamp + 1;
        let cleared = bridge.clear_before(boundary);
        assert_eq!(cleared, 1);
        assert_eq!(bridge.get_stats().total_entries, 1);
        assert_eq!(bridge.entries[0].content, "new entry");
    }

    #[test]
    fn test_tag_entry() {
        let mut bridge = default_bridge();
        let entry = bridge.ingest(ContextSource::Clipboard, "taggable").unwrap();

        bridge
            .tag_entry(&entry.id, vec!["important".to_string(), "work".to_string()])
            .unwrap();

        let updated = bridge.get_entry(&entry.id).unwrap();
        assert_eq!(updated.tags.len(), 2);
        assert!(updated.tags.contains(&"important".to_string()));

        // Duplicate tags should not be added
        bridge
            .tag_entry(&entry.id, vec!["important".to_string()])
            .unwrap();
        let updated2 = bridge.get_entry(&entry.id).unwrap();
        assert_eq!(updated2.tags.len(), 2);
    }

    #[test]
    fn test_stats_tracking() {
        let mut bridge = default_bridge();
        bridge
            .ingest(ContextSource::Clipboard, "contact user@test.com")
            .unwrap();
        bridge
            .ingest(
                ContextSource::Screen {
                    app_name: "Firefox".to_string(),
                    window_title: "Home".to_string(),
                },
                "no pii here",
            )
            .unwrap();

        let stats = bridge.get_stats();
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.entries_by_source.get("Clipboard"), Some(&1));
        assert_eq!(stats.entries_by_source.get("Screen"), Some(&1));
        assert_eq!(stats.total_pii_redacted, 1);
        assert!(stats.oldest_entry.is_some());
        assert!(stats.newest_entry.is_some());
        assert!(stats.storage_estimate_bytes > 0);
    }

    #[test]
    fn test_export_entries() {
        let mut bridge = default_bridge();
        bridge.ingest(ContextSource::Clipboard, "first").unwrap();
        bridge.ingest(ContextSource::Clipboard, "second").unwrap();

        // Backdate the first entry so the two have distinct timestamps.
        bridge.entries[0].timestamp -= 10;
        let ts_first = bridge.entries[0].timestamp;

        let all = bridge.export_entries(None);
        assert_eq!(all.len(), 2);

        let filtered = bridge.export_entries(Some((ts_first, ts_first)));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].content, "first");
    }
}
