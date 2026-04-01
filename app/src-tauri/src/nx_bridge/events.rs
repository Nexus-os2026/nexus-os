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

/// Consent request payload for the frontend consent modal.
#[derive(Debug, Clone, Serialize)]
pub struct NxConsentRequired {
    pub request_id: String,
    pub tool_name: String,
    pub tier: String,
    pub details: String,
    pub capability: String,
    pub fuel_cost: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NxDone {
    pub reason: String,
    pub total_turns: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct NxError {
    pub message: String,
}

/// Governance state update — emitted after each tool invocation.
#[derive(Debug, Clone, Serialize)]
pub struct NxGovernanceUpdate {
    pub fuel_remaining: u64,
    pub fuel_consumed: u64,
    pub audit_entries: usize,
    pub envelope_similarity: f64,
}
