//! AgentContext provides capability-gated, fuel-metered operations for agents.

use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use serde_json::json;
use uuid::Uuid;

const LLM_QUERY_FUEL_COST: u64 = 10;
const READ_FILE_FUEL_COST: u64 = 2;
const WRITE_FILE_FUEL_COST: u64 = 8;

#[derive(Debug, Clone)]
pub struct ApprovalRecord {
    pub description: String,
    pub requested_at: u64,
}

#[derive(Debug)]
pub struct AgentContext {
    agent_id: Uuid,
    capabilities: Vec<String>,
    fuel_budget: u64,
    fuel_remaining: u64,
    audit_trail: AuditTrail,
    approval_records: Vec<ApprovalRecord>,
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

    pub fn audit_trail(&self) -> &AuditTrail {
        &self.audit_trail
    }

    pub fn approval_records(&self) -> &[ApprovalRecord] {
        &self.approval_records
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
    pub fn llm_query(&mut self, prompt: &str, max_tokens: u32) -> Result<String, AgentError> {
        self.require_capability("llm.query")?;
        self.deduct_fuel(LLM_QUERY_FUEL_COST)?;

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
    pub fn read_file(&mut self, path: &str) -> Result<String, AgentError> {
        self.require_capability("fs.read")?;
        self.deduct_fuel(READ_FILE_FUEL_COST)?;

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
    pub fn write_file(&mut self, path: &str, content: &str) -> Result<(), AgentError> {
        self.require_capability("fs.write")?;
        self.deduct_fuel(WRITE_FILE_FUEL_COST)?;

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
    pub fn request_approval(&mut self, description: &str) -> ApprovalRecord {
        let record = ApprovalRecord {
            description: description.to_string(),
            requested_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };

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
}
