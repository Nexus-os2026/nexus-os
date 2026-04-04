//! Tauri event payloads emitted from the nx bridge to the frontend.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct NxTextDelta {
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NxToolStart {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NxToolComplete {
    pub name: String,
    pub success: bool,
    pub duration_ms: u64,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NxToolDenied {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NxConsentRequired {
    pub request_id: String,
    pub tool_name: String,
    pub tier: String,
    pub details: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NxDone {
    pub reason: String,
    pub total_turns: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct NxErrorEvent {
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NxGovernanceUpdate {
    pub fuel_remaining: u64,
    pub fuel_consumed: u64,
    pub audit_entries: usize,
}
