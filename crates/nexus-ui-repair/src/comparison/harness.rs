//! Phase 1.5 Group A — comparison harness.
//!
//! Compares a set of scout findings against a set of ground-truth
//! entries and produces a [`ComparisonReport`] with confirmed_match,
//! unknown_new, and confirmed_miss buckets plus an F1 score.

use serde::{Deserialize, Serialize};

use crate::ground_truth::parser::GroundTruthEntry;

/// A finding emitted by the scout — the minimal shape the comparison
/// harness needs. Real scout findings carry far more context; this is
/// a thin view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoutFinding {
    /// Optional GT-NNN id if the scout already believes this finding
    /// matches a ground-truth entry. None means "unknown_new candidate".
    pub matched_gt_id: Option<String>,
    /// Short human-readable label for reporting.
    pub label: String,
}

/// Result of comparing scout findings against the ground truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonReport {
    /// GT-NNN ids that the scout correctly matched.
    pub confirmed_match: Vec<String>,
    /// Scout findings with no matching GT-NNN entry.
    pub unknown_new: Vec<String>,
    /// GT-NNN ids that were in the ground truth but missed by the scout.
    pub confirmed_miss: Vec<String>,
    /// F1 score over confirmed_match vs. confirmed_miss vs. unknown_new.
    /// None when there is no signal (no matches and no misses).
    pub f1_score: Option<f64>,
}

impl ComparisonReport {
    /// Returns precision, or None when there were no positive predictions.
    pub fn precision(&self) -> Option<f64> {
        let tp = self.confirmed_match.len() as f64;
        let fp = self.unknown_new.len() as f64;
        if tp + fp == 0.0 {
            None
        } else {
            Some(tp / (tp + fp))
        }
    }

    /// Returns recall, or None when there were no ground-truth positives.
    pub fn recall(&self) -> Option<f64> {
        let tp = self.confirmed_match.len() as f64;
        let fn_count = self.confirmed_miss.len() as f64;
        if tp + fn_count == 0.0 {
            None
        } else {
            Some(tp / (tp + fn_count))
        }
    }
}

/// Compares scout findings against ground-truth entries. Ground-truth
/// entries whose status is "verified working" are excluded from both
/// the positive set and the miss set — they are baselines, not bugs.
pub fn compare(findings: &[ScoutFinding], ground_truth: &[GroundTruthEntry]) -> ComparisonReport {
    let bug_entries: Vec<&GroundTruthEntry> = ground_truth
        .iter()
        .filter(|e| e.status != "verified working")
        .collect();

    let mut confirmed_match: Vec<String> = Vec::new();
    let mut unknown_new: Vec<String> = Vec::new();

    for f in findings {
        match &f.matched_gt_id {
            Some(id) if bug_entries.iter().any(|e| &e.id == id) => {
                if !confirmed_match.contains(id) {
                    confirmed_match.push(id.clone());
                }
            }
            _ => unknown_new.push(f.label.clone()),
        }
    }

    let confirmed_miss: Vec<String> = bug_entries
        .iter()
        .filter(|e| !confirmed_match.contains(&e.id))
        .map(|e| e.id.clone())
        .collect();

    let tp = confirmed_match.len() as f64;
    let fp = unknown_new.len() as f64;
    let fn_count = confirmed_miss.len() as f64;

    let f1_score = if tp + fp + fn_count == 0.0 {
        None
    } else if tp == 0.0 {
        Some(0.0)
    } else {
        let precision = tp / (tp + fp);
        let recall = tp / (tp + fn_count);
        Some(2.0 * precision * recall / (precision + recall))
    };

    ComparisonReport {
        confirmed_match,
        unknown_new,
        confirmed_miss,
        f1_score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gt(id: &str, status: &str) -> GroundTruthEntry {
        GroundTruthEntry {
            id: id.into(),
            title: format!("{id} title"),
            where_location: String::new(),
            symptom: String::new(),
            expected: String::new(),
            hypothesis: String::new(),
            status: status.into(),
        }
    }

    fn finding(matched: Option<&str>, label: &str) -> ScoutFinding {
        ScoutFinding {
            matched_gt_id: matched.map(str::to_string),
            label: label.into(),
        }
    }

    #[test]
    fn test_perfect_match_gives_f1_one() {
        let gt_set = vec![gt("GT-001", ""), gt("GT-002", "")];
        let findings = vec![finding(Some("GT-001"), "a"), finding(Some("GT-002"), "b")];
        let report = compare(&findings, &gt_set);
        assert_eq!(report.confirmed_match.len(), 2);
        assert_eq!(report.unknown_new.len(), 0);
        assert_eq!(report.confirmed_miss.len(), 0);
        assert_eq!(report.f1_score, Some(1.0));
    }

    #[test]
    fn test_verified_working_baseline_excluded() {
        let gt_set = vec![gt("GT-001", ""), gt("GT-006", "verified working")];
        let findings = vec![finding(Some("GT-001"), "a")];
        let report = compare(&findings, &gt_set);
        // GT-006 must not count as a miss.
        assert!(!report.confirmed_miss.contains(&"GT-006".to_string()));
        assert_eq!(report.confirmed_miss.len(), 0);
        assert_eq!(report.f1_score, Some(1.0));
    }

    #[test]
    fn test_unknown_new_and_miss() {
        let gt_set = vec![gt("GT-001", ""), gt("GT-002", "")];
        let findings = vec![
            finding(Some("GT-001"), "match"),
            finding(None, "brand new thing"),
        ];
        let report = compare(&findings, &gt_set);
        assert_eq!(report.confirmed_match, vec!["GT-001".to_string()]);
        assert_eq!(report.unknown_new, vec!["brand new thing".to_string()]);
        assert_eq!(report.confirmed_miss, vec!["GT-002".to_string()]);
        // precision = 1/2, recall = 1/2, F1 = 0.5
        assert_eq!(report.f1_score, Some(0.5));
    }

    #[test]
    fn test_empty_inputs_f1_none() {
        let report = compare(&[], &[]);
        assert_eq!(report.f1_score, None);
    }

    #[test]
    fn test_precision_and_recall_helpers() {
        let gt_set = vec![gt("GT-001", ""), gt("GT-002", "")];
        let findings = vec![finding(Some("GT-001"), "a"), finding(None, "b")];
        let report = compare(&findings, &gt_set);
        assert_eq!(report.precision(), Some(0.5));
        assert_eq!(report.recall(), Some(0.5));
    }
}
