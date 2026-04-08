//! Vision-judge calibration log. See v1.1 amendment §8 Q3.
//!
//! The ambiguity-escalation threshold for `vision_judge` is STILL
//! OPEN in the amendment; the plan is to "calibrate from Phase 1
//! data." This module ships the **recording** side of that loop: a
//! JSONL append-only log that every `vision_judge` call writes to,
//! capturing the similarity score, threshold, verdict, and (later)
//! the human-verified ground truth.
//!
//! Phase 1.3 ships the recording API. Phase 1.4 ships the recompute
//! pass (see [`CalibrationLog::recompute_thresholds`]) and replaces
//! the placeholder timestamp with `chrono::Utc::now()`.

use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::governance::acl::Acl;

/// One calibration log entry. Appended as a single JSONL line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationEntry {
    pub call_id: String,
    /// RFC3339 timestamp. Phase 1.4 fills this from `chrono::Utc::now()`
    /// at construction time via [`CalibrationEntry::now`].
    pub timestamp: String,
    pub similarity: f64,
    pub threshold: f64,
    /// Serialized `VisionVerdict`. Kept as a plain string so the log
    /// format is stable across Phase 1.3/1.4 enum evolution.
    pub verdict: String,
    /// Filled in by a human later, during the calibration recompute
    /// pass (Phase 1.4+).
    pub ground_truth: Option<String>,
    /// Phase 1.4 addition: full classifier input as a JSON blob, so
    /// the recompute pass can re-run the rule table against candidate
    /// thresholds. Optional for backward compatibility — entries
    /// without it are skipped by `recompute_thresholds`.
    #[serde(default)]
    pub classifier_input: Option<serde_json::Value>,
}

impl CalibrationEntry {
    /// Build a calibration entry stamped with the current UTC time.
    pub fn now(
        call_id: impl Into<String>,
        similarity: f64,
        threshold: f64,
        verdict: impl Into<String>,
    ) -> Self {
        Self {
            call_id: call_id.into(),
            timestamp: Utc::now().to_rfc3339(),
            similarity,
            threshold,
            verdict: verdict.into(),
            ground_truth: None,
            classifier_input: None,
        }
    }
}

/// Append-only JSONL calibration log. Holds only the path; each
/// `record` call opens, appends, and closes.
#[derive(Debug, Clone)]
pub struct CalibrationLog {
    path: PathBuf,
}

impl CalibrationLog {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Append one entry. Creates the file if it does not exist.
    pub fn record(&self, entry: &CalibrationEntry) -> crate::Result<()> {
        let line = serde_json::to_string(entry)?;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)?;
        writeln!(f, "{}", line)?;
        Ok(())
    }

    /// Read every entry back. Missing files produce an empty vec.
    pub fn entries(&self) -> crate::Result<Vec<CalibrationEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let text = std::fs::read_to_string(&self.path)?;
        let mut out = Vec::new();
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            out.push(serde_json::from_str(line)?);
        }
        Ok(out)
    }

    /// The path this log writes to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Phase 1.4 Deliverable 6 — sweep candidate threshold pairs and
    /// pick the (min_action_window_ms, hang_threshold_ms) pair that
    /// maximizes F1 score against the human-labeled ground truth.
    ///
    /// `ground_truth_path` is a JSONL file where each line is a JSON
    /// object with fields `entry_id` (matching `CalibrationEntry.call_id`)
    /// and `true_label` (one of "Pass", "Dead", "Error", "Hang",
    /// "Ambiguous"). Entries from the calibration log are joined on
    /// `call_id == entry_id`. Entries that lack a `classifier_input`
    /// payload (legacy entries) or have no ground truth match are
    /// skipped.
    pub fn recompute_thresholds(
        &self,
        ground_truth_path: &Path,
        output_path: &Path,
    ) -> Result<RecomputeReport, CalibrationError> {
        if !ground_truth_path.exists() {
            return Err(CalibrationError::GroundTruthMissing);
        }
        let entries = self
            .entries()
            .map_err(|e| CalibrationError::IoError(e.to_string()))?;
        if entries.is_empty() {
            return Err(CalibrationError::CalibrationLogEmpty);
        }

        let gt_text = std::fs::read_to_string(ground_truth_path)
            .map_err(|e| CalibrationError::IoError(e.to_string()))?;
        let mut gt_map: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for line in gt_text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let parsed: serde_json::Value =
                serde_json::from_str(line).map_err(|e| CalibrationError::IoError(e.to_string()))?;
            let id = parsed["entry_id"]
                .as_str()
                .ok_or(CalibrationError::JoinFailed)?
                .to_string();
            let label = parsed["true_label"]
                .as_str()
                .ok_or(CalibrationError::JoinFailed)?
                .to_string();
            gt_map.insert(id, label);
        }

        // Join.
        let mut joined: Vec<(serde_json::Value, String)> = Vec::new();
        for entry in &entries {
            let Some(input_json) = entry.classifier_input.clone() else {
                continue;
            };
            let Some(label) = gt_map.get(&entry.call_id).cloned() else {
                continue;
            };
            joined.push((input_json, label));
        }
        if joined.is_empty() {
            return Err(CalibrationError::JoinFailed);
        }

        // Sweep.
        let mut best: Option<(u64, u64, f64)> = None;
        for min_window in (100u64..=2000).step_by(100) {
            for hang in (5_000u64..=30_000).step_by(1_000) {
                let f1 = score_thresholds(&joined, min_window, hang);
                let is_better = match best {
                    None => true,
                    Some((_, _, best_f1)) => f1 > best_f1,
                };
                if is_better {
                    best = Some((min_window, hang, f1));
                }
            }
        }
        let (min_window, hang, f1) = best.ok_or(CalibrationError::JoinFailed)?;

        let report = RecomputeReport {
            min_action_window_ms: min_window,
            hang_threshold_ms: hang,
            f1_score: f1,
            joined_entries: joined.len(),
        };

        // I-2 Layer 1: route parent-dir creation through the ACL helper.
        if let Some(parent) = output_path.parent() {
            if !parent.as_os_str().is_empty() {
                let acl = Acl::with_roots(vec![parent.to_path_buf()]);
                acl.ensure_parent_dirs(output_path)
                    .map_err(|e| CalibrationError::IoError(e.to_string()))?;
            }
        }
        let bytes = serde_json::to_vec_pretty(&report)
            .map_err(|e| CalibrationError::IoError(e.to_string()))?;
        std::fs::write(output_path, bytes).map_err(|e| CalibrationError::IoError(e.to_string()))?;

        Ok(report)
    }
}

/// Output of [`CalibrationLog::recompute_thresholds`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecomputeReport {
    pub min_action_window_ms: u64,
    pub hang_threshold_ms: u64,
    pub f1_score: f64,
    pub joined_entries: usize,
}

/// Errors raised by the recompute pass.
#[derive(Debug, thiserror::Error)]
pub enum CalibrationError {
    #[error("ground truth file missing")]
    GroundTruthMissing,
    #[error("calibration log is empty")]
    CalibrationLogEmpty,
    #[error("join produced zero rows (no calibration_input or no label match)")]
    JoinFailed,
    #[error("io error: {0}")]
    IoError(String),
}

/// Score a threshold pair against a joined dataset. Computes
/// macro-averaged F1 across the five labels and returns it.
fn score_thresholds(
    joined: &[(serde_json::Value, String)],
    min_action_window_ms: u64,
    hang_threshold_ms: u64,
) -> f64 {
    use crate::specialists::classifier::{Classification, ClassifierInput};

    let labels = ["Pass", "Dead", "Error", "Hang", "Ambiguous"];
    let mut tp = [0u64; 5];
    let mut fp = [0u64; 5];
    let mut fn_ = [0u64; 5];

    for (input_json, true_label) in joined {
        let Ok(input) = serde_json::from_value::<ClassifierInput>(input_json.clone()) else {
            continue;
        };
        let predicted = predict_with_thresholds(&input, min_action_window_ms, hang_threshold_ms);
        let predicted_str = match predicted {
            Classification::Pass => "Pass",
            Classification::Dead => "Dead",
            Classification::Error => "Error",
            Classification::Hang => "Hang",
            Classification::Ambiguous => "Ambiguous",
        };
        for (i, label) in labels.iter().enumerate() {
            let is_pred = predicted_str == *label;
            let is_true = true_label == *label;
            match (is_pred, is_true) {
                (true, true) => tp[i] += 1,
                (true, false) => fp[i] += 1,
                (false, true) => fn_[i] += 1,
                (false, false) => {}
            }
        }
    }

    let mut sum_f1 = 0.0;
    let mut counted = 0u64;
    for i in 0..5 {
        let denom_p = tp[i] + fp[i];
        let denom_r = tp[i] + fn_[i];
        if denom_p == 0 && denom_r == 0 {
            continue;
        }
        let precision = if denom_p == 0 {
            0.0
        } else {
            tp[i] as f64 / denom_p as f64
        };
        let recall = if denom_r == 0 {
            0.0
        } else {
            tp[i] as f64 / denom_r as f64
        };
        let f1 = if precision + recall == 0.0 {
            0.0
        } else {
            2.0 * precision * recall / (precision + recall)
        };
        sum_f1 += f1;
        counted += 1;
    }
    if counted == 0 {
        0.0
    } else {
        sum_f1 / counted as f64
    }
}

/// Re-implements the classifier rule table here so the sweep can
/// evaluate candidate thresholds without holding a `Classifier` (which
/// owns a calibration log mutex). The rules are kept in sync by hand;
/// a future refactor could lift them into a free function in
/// `classifier.rs`.
pub fn predict_with_thresholds(
    input: &crate::specialists::classifier::ClassifierInput,
    min_action_window_ms: u64,
    hang_threshold_ms: u64,
) -> crate::specialists::classifier::Classification {
    use crate::specialists::classifier::Classification;
    use crate::specialists::vision_judge::VisionVerdictKind;

    if !input.console_errors.is_empty() {
        return Classification::Error;
    }
    if input.elapsed_ms > hang_threshold_ms && !input.signal_change_after_action {
        return Classification::Hang;
    }
    if input.dom_mutations.is_empty()
        && input.ipc_traffic.is_empty()
        && matches!(input.vision_verdict.verdict, VisionVerdictKind::Unchanged)
    {
        return Classification::Dead;
    }
    if matches!(input.vision_verdict.verdict, VisionVerdictKind::Changed)
        && !input.dom_mutations.is_empty()
        && input.elapsed_ms >= min_action_window_ms
    {
        return Classification::Pass;
    }
    Classification::Ambiguous
}
