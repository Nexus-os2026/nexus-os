//! Deploy Diff — compare file manifests between two deploys.
//!
//! Compares by path + hash. Same path/different hash → modified.
//! Only in `to` → added. Only in `from` → removed.
//! No full file content comparison — hash-based only.

use super::history::FileManifestEntry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Diff between two deploys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployDiff {
    pub from_id: String,
    pub to_id: String,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<String>,
    pub unchanged: usize,
    pub from_hash: String,
    pub to_hash: String,
}

/// Compute the diff between two file manifests.
pub fn compute_diff(
    from_files: &[FileManifestEntry],
    to_files: &[FileManifestEntry],
    from_id: &str,
    to_id: &str,
    from_hash: &str,
    to_hash: &str,
) -> DeployDiff {
    let from_map: HashMap<&str, &str> = from_files
        .iter()
        .map(|f| (f.path.as_str(), f.hash.as_str()))
        .collect();
    let to_map: HashMap<&str, &str> = to_files
        .iter()
        .map(|f| (f.path.as_str(), f.hash.as_str()))
        .collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();
    let mut unchanged = 0usize;

    // Check all files in `to` against `from`
    for (path, to_h) in &to_map {
        match from_map.get(path) {
            Some(from_h) => {
                if from_h == to_h {
                    unchanged += 1;
                } else {
                    modified.push(path.to_string());
                }
            }
            None => {
                added.push(path.to_string());
            }
        }
    }

    // Check for files removed (in `from` but not in `to`)
    for path in from_map.keys() {
        if !to_map.contains_key(path) {
            removed.push(path.to_string());
        }
    }

    added.sort();
    removed.sort();
    modified.sort();

    DeployDiff {
        from_id: from_id.into(),
        to_id: to_id.into(),
        added,
        removed,
        modified,
        unchanged,
        from_hash: from_hash.into(),
        to_hash: to_hash.into(),
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, hash: &str) -> FileManifestEntry {
        FileManifestEntry {
            path: path.into(),
            hash: hash.into(),
            size: 100,
        }
    }

    #[test]
    fn test_diff_added_files() {
        let from = vec![file("a.html", "h1")];
        let to = vec![file("a.html", "h1"), file("b.html", "h2")];
        let diff = compute_diff(&from, &to, "d1", "d2", "bh1", "bh2");
        assert_eq!(diff.added, vec!["b.html"]);
        assert!(diff.removed.is_empty());
        assert!(diff.modified.is_empty());
        assert_eq!(diff.unchanged, 1);
    }

    #[test]
    fn test_diff_removed_files() {
        let from = vec![file("a.html", "h1"), file("b.html", "h2")];
        let to = vec![file("a.html", "h1")];
        let diff = compute_diff(&from, &to, "d1", "d2", "bh1", "bh2");
        assert!(diff.added.is_empty());
        assert_eq!(diff.removed, vec!["b.html"]);
        assert_eq!(diff.unchanged, 1);
    }

    #[test]
    fn test_diff_modified_files() {
        let from = vec![file("a.html", "h1"), file("style.css", "old")];
        let to = vec![file("a.html", "h1"), file("style.css", "new")];
        let diff = compute_diff(&from, &to, "d1", "d2", "bh1", "bh2");
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert_eq!(diff.modified, vec!["style.css"]);
        assert_eq!(diff.unchanged, 1);
    }

    #[test]
    fn test_diff_unchanged_count() {
        let files = vec![
            file("a.html", "h1"),
            file("b.css", "h2"),
            file("c.js", "h3"),
        ];
        let diff = compute_diff(&files, &files, "d1", "d2", "bh1", "bh2");
        assert_eq!(diff.unchanged, 3);
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn test_diff_identical_deploys() {
        let files = vec![file("index.html", "abc")];
        let diff = compute_diff(&files, &files, "d1", "d2", "same", "same");
        assert_eq!(diff.unchanged, 1);
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn test_diff_empty_from() {
        let to = vec![file("a.html", "h1"), file("b.css", "h2")];
        let diff = compute_diff(&[], &to, "d1", "d2", "bh1", "bh2");
        assert_eq!(diff.added.len(), 2);
        assert!(diff.removed.is_empty());
        assert_eq!(diff.unchanged, 0);
    }
}
