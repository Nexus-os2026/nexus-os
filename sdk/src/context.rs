//! AgentContext provides capability-gated, fuel-metered operations for agents.

use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

const LLM_QUERY_FUEL_COST: u64 = 10;
const READ_FILE_FUEL_COST: u64 = 2;
const WRITE_FILE_FUEL_COST: u64 = 8;

/// A side-effect captured when `AgentContext` is in recording mode.
///
/// Instead of executing the real operation, the context logs what *would*
/// happen. This is used by `ShadowSandbox` to capture speculative effects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContextSideEffect {
    /// LLM query attempted.
    LlmQuery {
        prompt: String,
        max_tokens: u32,
        fuel_cost: u64,
    },
    /// File read attempted.
    FileRead { path: String, fuel_cost: u64 },
    /// File write attempted.
    FileWrite {
        path: String,
        content_size: usize,
        fuel_cost: u64,
    },
    /// Approval requested.
    ApprovalRequest { description: String },
    /// Audit event emitted.
    AuditEvent { payload: serde_json::Value },
}

#[derive(Debug, Clone)]
pub struct ApprovalRecord {
    pub description: String,
    pub requested_at: u64,
}

#[derive(Debug, Clone)]
pub struct AgentContext {
    agent_id: Uuid,
    capabilities: Vec<String>,
    fuel_budget: u64,
    fuel_remaining: u64,
    audit_trail: AuditTrail,
    approval_records: Vec<ApprovalRecord>,
    recording_mode: bool,
    side_effect_log: Vec<ContextSideEffect>,
}

impl AgentContext {
    pub fn new(agent_id: Uuid, capabilities: Vec<String>, fuel_budget: u64) -> Self {
        Self {
            agent_id,
            capabilities,
            fuel_budget,
            fuel_remaining: fuel_budget,
            audit_trail: AuditTrail::new(),
            approval_records: Vec::new(),
            recording_mode: false,
            side_effect_log: Vec::new(),
        }
    }

    pub fn agent_id(&self) -> Uuid {
        self.agent_id
    }

    pub fn fuel_remaining(&self) -> u64 {
        self.fuel_remaining
    }

    pub fn fuel_budget(&self) -> u64 {
        self.fuel_budget
    }

    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }

    pub fn audit_trail(&self) -> &AuditTrail {
        &self.audit_trail
    }

    pub fn audit_trail_mut(&mut self) -> &mut AuditTrail {
        &mut self.audit_trail
    }

    pub fn approval_records(&self) -> &[ApprovalRecord] {
        &self.approval_records
    }

    /// Enable recording mode. Operations push to `side_effect_log` instead of
    /// executing. Capability and fuel checks still apply — only the real
    /// action is skipped.
    pub fn enable_recording(&mut self) {
        self.recording_mode = true;
    }

    /// Disable recording mode. Operations resume normal execution.
    pub fn disable_recording(&mut self) {
        self.recording_mode = false;
    }

    /// Whether the context is in recording mode.
    pub fn is_recording(&self) -> bool {
        self.recording_mode
    }

    /// Read-only access to accumulated side-effects.
    pub fn side_effects(&self) -> &[ContextSideEffect] {
        &self.side_effect_log
    }

    /// Drain and return all captured side-effects, clearing the log.
    pub fn drain_side_effects(&mut self) -> Vec<ContextSideEffect> {
        std::mem::take(&mut self.side_effect_log)
    }

    /// Manually record a side-effect (used by speculative policy interception
    /// in host functions to log what *would* have happened).
    pub fn record_side_effect(&mut self, effect: ContextSideEffect) {
        self.side_effect_log.push(effect);
    }

    /// Check that a capability is in the manifest. Returns AgentError::CapabilityDenied if not.
    pub fn require_capability(&self, capability: &str) -> Result<(), AgentError> {
        if self.capabilities.contains(&capability.to_string()) {
            Ok(())
        } else {
            Err(AgentError::CapabilityDenied(capability.to_string()))
        }
    }

    /// Query an LLM. Checks "llm.query" capability and deducts fuel.
    /// In recording mode, logs the side-effect and returns a placeholder.
    pub fn llm_query(&mut self, prompt: &str, max_tokens: u32) -> Result<String, AgentError> {
        self.require_capability("llm.query")?;
        self.deduct_fuel(LLM_QUERY_FUEL_COST)?;

        if self.recording_mode {
            self.side_effect_log.push(ContextSideEffect::LlmQuery {
                prompt: prompt.to_string(),
                max_tokens,
                fuel_cost: LLM_QUERY_FUEL_COST,
            });
            return Ok(format!(
                "[recorded-llm-query: {} chars, max_tokens={}]",
                prompt.len(),
                max_tokens
            ));
        }

        self.audit_trail.append_event(
            self.agent_id,
            EventType::LlmCall,
            json!({
                "action": "llm_query",
                "prompt_len": prompt.len(),
                "max_tokens": max_tokens,
                "fuel_cost": LLM_QUERY_FUEL_COST,
            }),
        );

        Ok(format!("[mock-llm-response to {} chars]", prompt.len()))
    }

    /// Read a file. Checks "fs.read" capability, costs 2 fuel.
    /// In recording mode, logs the side-effect and returns a placeholder.
    pub fn read_file(&mut self, path: &str) -> Result<String, AgentError> {
        self.require_capability("fs.read")?;
        self.deduct_fuel(READ_FILE_FUEL_COST)?;

        if self.recording_mode {
            self.side_effect_log.push(ContextSideEffect::FileRead {
                path: path.to_string(),
                fuel_cost: READ_FILE_FUEL_COST,
            });
            return Ok(format!("[recorded-file-read: {}]", path));
        }

        self.audit_trail.append_event(
            self.agent_id,
            EventType::ToolCall,
            json!({
                "action": "read_file",
                "path": path,
                "fuel_cost": READ_FILE_FUEL_COST,
            }),
        );

        Ok(format!("[mock-file-content of {}]", path))
    }

    /// Write a file. Checks "fs.write" capability, costs 8 fuel.
    /// In recording mode, logs the side-effect instead of writing.
    pub fn write_file(&mut self, path: &str, content: &str) -> Result<(), AgentError> {
        self.require_capability("fs.write")?;
        self.deduct_fuel(WRITE_FILE_FUEL_COST)?;

        if self.recording_mode {
            self.side_effect_log.push(ContextSideEffect::FileWrite {
                path: path.to_string(),
                content_size: content.len(),
                fuel_cost: WRITE_FILE_FUEL_COST,
            });
            return Ok(());
        }

        self.audit_trail.append_event(
            self.agent_id,
            EventType::ToolCall,
            json!({
                "action": "write_file",
                "path": path,
                "content_len": content.len(),
                "fuel_cost": WRITE_FILE_FUEL_COST,
            }),
        );

        Ok(())
    }

    /// Request approval for a described action.
    /// In recording mode, logs the side-effect instead of recording approval.
    pub fn request_approval(&mut self, description: &str) -> ApprovalRecord {
        let record = ApprovalRecord {
            description: description.to_string(),
            requested_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };

        if self.recording_mode {
            self.side_effect_log
                .push(ContextSideEffect::ApprovalRequest {
                    description: description.to_string(),
                });
            return record;
        }

        self.audit_trail.append_event(
            self.agent_id,
            EventType::UserAction,
            json!({
                "action": "request_approval",
                "description": description,
            }),
        );

        self.approval_records.push(record.clone());
        record
    }

    /// Deduct fuel consumed by wasm execution (instruction-level cost).
    /// Called by WasmtimeSandbox after execution to sync fuel state back.
    /// This is separate from per-operation costs (llm_query, read_file, etc.)
    /// which are already deducted by the respective AgentContext methods.
    pub fn deduct_wasm_fuel(&mut self, units: u64) {
        self.fuel_remaining = self.fuel_remaining.saturating_sub(units);
        if units > 0 {
            self.audit_trail.append_event(
                self.agent_id,
                EventType::ToolCall,
                json!({
                    "action": "wasm_fuel_consumed",
                    "units": units,
                    "remaining": self.fuel_remaining,
                }),
            );
        }
    }

    fn deduct_fuel(&mut self, cost: u64) -> Result<(), AgentError> {
        if self.fuel_remaining < cost {
            self.audit_trail.append_event(
                self.agent_id,
                EventType::Error,
                json!({
                    "action": "fuel_exhausted",
                    "requested": cost,
                    "remaining": self.fuel_remaining,
                }),
            );
            return Err(AgentError::FuelExhausted);
        }
        self.fuel_remaining -= cost;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_check_blocks_unauthorized() {
        let ctx = AgentContext::new(Uuid::new_v4(), vec!["fs.read".to_string()], 1000);

        assert!(ctx.require_capability("fs.read").is_ok());
        assert!(matches!(
            ctx.require_capability("llm.query"),
            Err(AgentError::CapabilityDenied(_))
        ));
    }

    #[test]
    fn fuel_deduction_and_exhaustion() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["llm.query".to_string()], 15);

        // First query costs 10
        assert!(ctx.llm_query("test", 100).is_ok());
        assert_eq!(ctx.fuel_remaining(), 5);

        // Second query would cost 10 but only 5 left
        let result = ctx.llm_query("test2", 100);
        assert!(matches!(result, Err(AgentError::FuelExhausted)));
        assert_eq!(ctx.fuel_remaining(), 5);
    }

    #[test]
    fn operations_emit_audit_events() {
        let mut ctx = AgentContext::new(
            Uuid::new_v4(),
            vec![
                "llm.query".to_string(),
                "fs.read".to_string(),
                "fs.write".to_string(),
            ],
            1000,
        );

        ctx.llm_query("prompt", 50).unwrap();
        ctx.read_file("/tmp/test.txt").unwrap();
        ctx.write_file("/tmp/out.txt", "data").unwrap();
        ctx.request_approval("deploy to production");

        assert_eq!(ctx.audit_trail().events().len(), 4);
        assert_eq!(ctx.approval_records().len(), 1);
    }

    #[test]
    fn read_file_checks_capability() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["llm.query".to_string()], 1000);

        let result = ctx.read_file("/etc/passwd");
        assert!(matches!(result, Err(AgentError::CapabilityDenied(_))));
    }

    #[test]
    fn write_file_checks_capability_and_fuel() {
        let mut ctx = AgentContext::new(
            Uuid::new_v4(),
            vec!["fs.write".to_string()],
            5, // less than WRITE_FILE_FUEL_COST (8)
        );

        let result = ctx.write_file("/tmp/out.txt", "data");
        assert!(matches!(result, Err(AgentError::FuelExhausted)));
    }

    #[test]
    fn recording_mode_defaults_to_false() {
        let ctx = AgentContext::new(Uuid::new_v4(), vec![], 1000);
        assert!(!ctx.is_recording());
        assert!(ctx.side_effects().is_empty());
    }

    #[test]
    fn recording_mode_captures_llm_query() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["llm.query".to_string()], 1000);
        ctx.enable_recording();

        let result = ctx.llm_query("hello world", 50).unwrap();
        assert!(result.starts_with("[recorded-llm-query:"));
        assert_eq!(ctx.side_effects().len(), 1);
        assert!(matches!(
            &ctx.side_effects()[0],
            ContextSideEffect::LlmQuery { prompt, max_tokens: 50, fuel_cost: 10 }
            if prompt == "hello world"
        ));
        // Fuel is still deducted in recording mode
        assert_eq!(ctx.fuel_remaining(), 990);
        // Audit trail should NOT have the event
        assert_eq!(ctx.audit_trail().events().len(), 0);
    }

    #[test]
    fn recording_mode_captures_file_read() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["fs.read".to_string()], 1000);
        ctx.enable_recording();

        let result = ctx.read_file("/etc/hosts").unwrap();
        assert!(result.starts_with("[recorded-file-read:"));
        assert_eq!(ctx.side_effects().len(), 1);
        assert!(matches!(
            &ctx.side_effects()[0],
            ContextSideEffect::FileRead { path, fuel_cost: 2 }
            if path == "/etc/hosts"
        ));
    }

    #[test]
    fn recording_mode_captures_file_write() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["fs.write".to_string()], 1000);
        ctx.enable_recording();

        ctx.write_file("/tmp/out.txt", "some data").unwrap();
        assert_eq!(ctx.side_effects().len(), 1);
        assert!(matches!(
            &ctx.side_effects()[0],
            ContextSideEffect::FileWrite { path, content_size: 9, fuel_cost: 8 }
            if path == "/tmp/out.txt"
        ));
    }

    #[test]
    fn recording_mode_captures_approval_request() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec![], 1000);
        ctx.enable_recording();

        ctx.request_approval("deploy to prod");
        assert_eq!(ctx.side_effects().len(), 1);
        assert!(matches!(
            &ctx.side_effects()[0],
            ContextSideEffect::ApprovalRequest { description }
            if description == "deploy to prod"
        ));
        // Should NOT add to approval_records in recording mode
        assert_eq!(ctx.approval_records().len(), 0);
    }

    #[test]
    fn recording_mode_still_checks_capabilities() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec![], 1000);
        ctx.enable_recording();

        let result = ctx.llm_query("test", 50);
        assert!(matches!(result, Err(AgentError::CapabilityDenied(_))));
        assert!(ctx.side_effects().is_empty());
    }

    #[test]
    fn recording_mode_still_checks_fuel() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["llm.query".to_string()], 5);
        ctx.enable_recording();

        let result = ctx.llm_query("test", 50);
        assert!(matches!(result, Err(AgentError::FuelExhausted)));
        assert!(ctx.side_effects().is_empty());
    }

    #[test]
    fn drain_side_effects_clears_log() {
        let mut ctx = AgentContext::new(
            Uuid::new_v4(),
            vec!["llm.query".to_string(), "fs.read".to_string()],
            1000,
        );
        ctx.enable_recording();

        ctx.llm_query("test", 50).unwrap();
        ctx.read_file("/tmp/x").unwrap();
        assert_eq!(ctx.side_effects().len(), 2);

        let drained = ctx.drain_side_effects();
        assert_eq!(drained.len(), 2);
        assert!(ctx.side_effects().is_empty());
    }

    #[test]
    fn disable_recording_resumes_normal_execution() {
        let mut ctx = AgentContext::new(
            Uuid::new_v4(),
            vec!["llm.query".to_string()],
            1000,
        );
        ctx.enable_recording();
        ctx.llm_query("recorded", 50).unwrap();
        assert_eq!(ctx.side_effects().len(), 1);
        assert_eq!(ctx.audit_trail().events().len(), 0);

        ctx.disable_recording();
        ctx.llm_query("normal", 50).unwrap();
        // Side-effect log unchanged (no new recording)
        assert_eq!(ctx.side_effects().len(), 1);
        // Audit trail now has the event
        assert_eq!(ctx.audit_trail().events().len(), 1);
    }
}
