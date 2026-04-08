//! SpecialistCall — the (inputs, output) tuple recorded for every
//! specialist invocation. See v1.1 amendment §4.1.
//!
//! I-5 (Replayable sessions) requires that the audit log capture not
//! just the inputs to every specialist call but also the output. That
//! is what makes replay byte-identical despite non-deterministic LLM
//! calls in `vision_judge`: we never re-run the specialist during
//! replay, we read the recorded output back.
//!
//! Phase 1.3 ships the type and the audit-log integration
//! (`AuditLog::record_specialist_call`). Phase 1.4 wires the driver
//! loop to actually construct and record one of these for every
//! specialist invocation. As of Phase 1.4 the timestamp is a real
//! `chrono::Utc::now()` RFC3339 string.

use chrono::Utc;
use serde::{Deserialize, Serialize};

/// One specialist call: (specialist_name, inputs, output).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialistCall {
    pub specialist_name: String,
    pub timestamp: String,
    pub inputs: serde_json::Value,
    pub output: serde_json::Value,
}

impl SpecialistCall {
    /// Construct a call timestamped with `chrono::Utc::now()` in
    /// RFC3339 form.
    pub fn new(
        name: impl Into<String>,
        inputs: serde_json::Value,
        output: serde_json::Value,
    ) -> Self {
        Self {
            specialist_name: name.into(),
            timestamp: Utc::now().to_rfc3339(),
            inputs,
            output,
        }
    }
}
