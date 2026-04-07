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
//! specialist invocation. The timestamp field is a placeholder until
//! chrono is wired in Phase 1.4.

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
    /// Construct a call with the Phase 1.3 placeholder timestamp.
    /// Phase 1.4 will replace this with a real chrono-produced string.
    pub fn new(
        name: impl Into<String>,
        inputs: serde_json::Value,
        output: serde_json::Value,
    ) -> Self {
        Self {
            specialist_name: name.into(),
            timestamp: "2026-04-07T12:00:00Z".to_string(),
            inputs,
            output,
        }
    }
}
