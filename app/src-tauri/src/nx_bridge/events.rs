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

// ─── Computer Use Agent Events ───
// These structs are emitted dynamically via `app_handle.emit()` during agent runs.
// Some are reserved for future granular event emission as the agent loop matures.

#[derive(Debug, Clone, Serialize)]
pub struct NxAgentStepStarted {
    pub step: u32,
    pub max_steps: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct NxAgentScreenshot {
    pub base64: String,
    pub width: u32,
    pub height: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct NxAgentPlanReady {
    pub reasoning: String,
    pub actions: Vec<String>,
    pub confidence: f32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct NxAgentApprovalNeeded {
    pub step: u32,
    pub reasoning: String,
    pub actions: Vec<String>,
    pub confidence: f32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct NxAgentActionExecuted {
    pub action: String,
    pub success: bool,
    pub audit_hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NxAgentComplete {
    pub summary: String,
    pub steps: u32,
    pub fuel: u64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct NxAgentError {
    pub message: String,
}
