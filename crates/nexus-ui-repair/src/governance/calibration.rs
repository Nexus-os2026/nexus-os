//! Vision-judge calibration log. See v1.1 amendment §8 Q3.
//!
//! The ambiguity-escalation threshold for `vision_judge` is STILL
//! OPEN in the amendment; the plan is to "calibrate from Phase 1
//! data." This module ships the **recording** side of that loop: a
//! JSONL append-only log that every `vision_judge` call writes to,
//! capturing the similarity score, threshold, verdict, and (later)
//! the human-verified ground truth.
//!
//! Phase 1.3 only ships the recording API. Calibration recompute —
//! reading the log, comparing against ground-truth labels, and
//! producing a new threshold — lands in Phase 1.4 when `vision_judge`
//! actually runs. The timestamp field is a placeholder until chrono
//! is wired in Phase 1.4.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One calibration log entry. Appended as a single JSONL line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationEntry {
    pub call_id: String,
    /// Placeholder until Phase 1.4 wires chrono. Phase 1.3 uses the
    /// fixed string `"2026-04-07T12:00:00Z"` unless the caller
    /// overrides it.
    pub timestamp: String,
    pub similarity: f64,
    pub threshold: f64,
    /// Serialized `VisionVerdict`. Kept as a plain string so the log
    /// format is stable across Phase 1.3/1.4 enum evolution.
    pub verdict: String,
    /// Filled in by a human later, during the calibration recompute
    /// pass (Phase 1.4+).
    pub ground_truth: Option<String>,
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
}
