//! Phase 1.4 Deliverable 5 — deterministic 5-rule classifier.
//!
//! Maps a bundle of post-action signals (vision verdict, console
//! errors, IPC traffic, DOM mutations, elapsed time, post-action
//! signal change) to one of five [`Classification`] labels via a
//! fixed rule table evaluated in order.
//!
//! Every classification is appended to the calibration log so a later
//! [`crate::governance::CalibrationLog::recompute_thresholds`] pass
//! can tune `min_action_window_ms` and `hang_threshold_ms` against
//! human-labeled ground truth.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::governance::calibration::{CalibrationEntry, CalibrationLog};
use crate::specialists::vision_judge::{VisionVerdict, VisionVerdictKind};

/// Default minimum window between an action and a post-action signal
/// before the classifier will consider a `Pass`. 500ms is the Phase
/// 1.4 starting point; the calibration recompute pass tunes it.
pub const DEFAULT_MIN_ACTION_WINDOW_MS: u64 = 500;

/// Default elapsed-time threshold above which an action with no
/// signal change is classified as `Hang`. 10s is the Phase 1.4
/// starting point; the calibration recompute pass tunes it.
pub const DEFAULT_HANG_THRESHOLD_MS: u64 = 10_000;

/// One IPC event observed in the post-action window. Phase 1.4 keeps
/// this opaque — the classifier only cares whether the slice is
/// empty.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcEvent {
    pub command: String,
}

/// One DOM mutation observed in the post-action window. Same as
/// [`IpcEvent`]: opaque to the classifier, only emptiness matters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomMutation {
    pub selector: String,
}

/// Bundle of signals fed into [`Classifier::classify`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifierInput {
    pub vision_verdict: VisionVerdict,
    pub console_errors: Vec<String>,
    pub ipc_traffic: Vec<IpcEvent>,
    pub dom_mutations: Vec<DomMutation>,
    pub elapsed_ms: u64,
    /// True iff *any* signal (DOM, IPC, vision) changed after the
    /// action was issued. Used by the Hang rule to distinguish a slow
    /// success from a true hang.
    pub signal_change_after_action: bool,
}

/// One of the v1.1 classification labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Classification {
    Pass,
    Dead,
    Error,
    Hang,
    Ambiguous,
}

impl Classification {
    fn as_str(&self) -> &'static str {
        match self {
            Classification::Pass => "Pass",
            Classification::Dead => "Dead",
            Classification::Error => "Error",
            Classification::Hang => "Hang",
            Classification::Ambiguous => "Ambiguous",
        }
    }
}

/// The deterministic-rules classifier.
#[derive(Debug, Clone)]
pub struct Classifier {
    pub min_action_window_ms: u64,
    pub hang_threshold_ms: u64,
    pub calibration_log: Arc<Mutex<CalibrationLog>>,
}

impl Classifier {
    /// Construct a classifier with the Phase 1.4 default thresholds.
    pub fn new(calibration_log: Arc<Mutex<CalibrationLog>>) -> Self {
        Self {
            min_action_window_ms: DEFAULT_MIN_ACTION_WINDOW_MS,
            hang_threshold_ms: DEFAULT_HANG_THRESHOLD_MS,
            calibration_log,
        }
    }

    /// Construct a classifier with explicit thresholds (calibration
    /// recompute uses this with sweep-search candidate values).
    pub fn with_thresholds(
        min_action_window_ms: u64,
        hang_threshold_ms: u64,
        calibration_log: Arc<Mutex<CalibrationLog>>,
    ) -> Self {
        Self {
            min_action_window_ms,
            hang_threshold_ms,
            calibration_log,
        }
    }

    /// Apply the 5-rule table in order. Logs the result to the
    /// calibration log on success; a calibration log write failure is
    /// swallowed (logged via `tracing::warn`) so the driver loop is
    /// never blocked by an audit-side concern.
    pub fn classify(&self, input: &ClassifierInput) -> Classification {
        let result = self.evaluate(input);
        self.record_calibration(input, result);
        result
    }

    /// Pure rule evaluation, no side effects. Exposed for testing the
    /// calibration recompute sweep without touching the log.
    pub fn evaluate(&self, input: &ClassifierInput) -> Classification {
        // Rule 1: any console error wins.
        if !input.console_errors.is_empty() {
            return Classification::Error;
        }
        // Rule 2: hang — long elapsed with no post-action signal.
        if input.elapsed_ms > self.hang_threshold_ms && !input.signal_change_after_action {
            return Classification::Hang;
        }
        // Rule 3: dead — nothing happened anywhere.
        if input.dom_mutations.is_empty()
            && input.ipc_traffic.is_empty()
            && matches!(input.vision_verdict.verdict, VisionVerdictKind::Unchanged)
        {
            return Classification::Dead;
        }
        // Rule 4: pass — vision saw a change, DOM mutated, action
        // window long enough that it's not a flicker.
        if matches!(input.vision_verdict.verdict, VisionVerdictKind::Changed)
            && !input.dom_mutations.is_empty()
            && input.elapsed_ms >= self.min_action_window_ms
        {
            return Classification::Pass;
        }
        // Rule 5: everything else.
        Classification::Ambiguous
    }

    fn record_calibration(&self, input: &ClassifierInput, result: Classification) {
        let entry = CalibrationEntry::now(
            format!(
                "classify_{}",
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ),
            // similarity is not directly produced by the rule table;
            // record the elapsed_ms normalized to seconds as a proxy
            // signal that the recompute pass can ignore.
            input.elapsed_ms as f64 / 1000.0,
            self.min_action_window_ms as f64 / 1000.0,
            result.as_str(),
        );
        if let Ok(log) = self.calibration_log.lock() {
            if let Err(e) = log.record(&entry) {
                tracing::warn!(error = %e, "calibration log record failed");
            }
        }
    }
}
