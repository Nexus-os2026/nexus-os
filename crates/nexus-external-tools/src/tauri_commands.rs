//! Frontend integration types.

use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::audit::ToolAuditTrail;
use crate::execution::{ToolCallResult, ToolExecutionEngine};
use crate::governance::ToolGovernancePolicy;
use crate::registry::{ExternalTool, ToolRegistry};

/// In-memory state held by the Tauri app.
pub struct ToolState {
    pub engine: RwLock<ToolExecutionEngine>,
    pub audit: RwLock<ToolAuditTrail>,
    pub policy: ToolGovernancePolicy,
}

impl Default for ToolState {
    fn default() -> Self {
        let policy = ToolGovernancePolicy::default();
        Self {
            engine: RwLock::new(ToolExecutionEngine::new(
                ToolRegistry::default_registry(),
                policy.clone(),
            )),
            audit: RwLock::new(ToolAuditTrail::new()),
            policy,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStatus {
    pub tool_id: String,
    pub calls_this_minute: u32,
    pub max_per_minute: u32,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

pub fn tools_list_available(state: &ToolState) -> Result<Vec<ExternalTool>, String> {
    let engine = state.engine.read().map_err(|e| format!("lock: {e}"))?;
    Ok(engine
        .registry()
        .available_tools()
        .into_iter()
        .cloned()
        .collect())
}

pub fn tools_execute(
    state: &ToolState,
    agent_id: &str,
    autonomy_level: u8,
    tool_id: &str,
    params_json: &str,
) -> Result<ToolCallResult, String> {
    let params: serde_json::Value =
        serde_json::from_str(params_json).map_err(|e| format!("Invalid params: {e}"))?;

    let action = params
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let result = state
        .engine
        .write()
        .map_err(|e| format!("lock: {e}"))?
        .execute(agent_id, autonomy_level, tool_id, params)
        .map_err(|e| e.to_string())?;

    // Record in audit
    if let Ok(mut audit) = state.audit.write() {
        audit.record(&result, &action);
    }

    Ok(result)
}

pub fn tools_get_registry(state: &ToolState) -> Result<Vec<ExternalTool>, String> {
    let engine = state.engine.read().map_err(|e| format!("lock: {e}"))?;
    Ok(engine.registry().all_tools().to_vec())
}

pub fn tools_refresh_availability(state: &ToolState) -> Result<Vec<ExternalTool>, String> {
    let mut engine = state.engine.write().map_err(|e| format!("lock: {e}"))?;
    engine.registry_mut().refresh_availability();
    Ok(engine.registry().all_tools().to_vec())
}

pub fn tools_get_audit(
    state: &ToolState,
    limit: usize,
) -> Result<Vec<crate::audit::ToolAuditEntry>, String> {
    let audit = state.audit.read().map_err(|e| format!("lock: {e}"))?;
    let entries = audit.entries();
    let start = entries.len().saturating_sub(limit);
    Ok(entries[start..].to_vec())
}

pub fn tools_verify_audit(state: &ToolState) -> Result<bool, String> {
    let audit = state.audit.read().map_err(|e| format!("lock: {e}"))?;
    match audit.verify_chain() {
        Ok(()) => Ok(true),
        Err(e) => Err(e),
    }
}

pub fn tools_get_policy(state: &ToolState) -> ToolGovernancePolicy {
    state.policy.clone()
}
