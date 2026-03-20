//! Instrumentation span builders for Nexus OS critical paths.
//!
//! Provides structured span recording that integrates with the `tracing` crate.
//! Each span records attributes following the OpenTelemetry semantic conventions
//! adapted for Nexus OS governance primitives.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Outcome of a capability or fuel check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckResult {
    Granted,
    Denied,
    Sufficient,
    Exhausted,
}

impl std::fmt::Display for CheckResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Granted => write!(f, "granted"),
            Self::Denied => write!(f, "denied"),
            Self::Sufficient => write!(f, "sufficient"),
            Self::Exhausted => write!(f, "exhausted"),
        }
    }
}

/// HITL gate decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HitlDecision {
    Approved,
    Denied,
    Timeout,
}

impl std::fmt::Display for HitlDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Approved => write!(f, "approved"),
            Self::Denied => write!(f, "denied"),
            Self::Timeout => write!(f, "timeout"),
        }
    }
}

/// Agent execution span attributes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecutionSpan {
    pub agent_did: String,
    pub autonomy_level: u8,
    pub task_type: String,
    pub capability_checks: Vec<CapabilityCheckSpan>,
    pub fuel_check: Option<FuelCheckSpan>,
    pub hitl_gate: Option<HitlGateSpan>,
    pub sandbox_execution: Option<SandboxSpan>,
    pub llm_request: Option<LlmRequestSpan>,
    pub pii_redaction: Option<PiiRedactionSpan>,
    pub audit_write: Option<AuditWriteSpan>,
    pub duration_ms: u64,
    pub status: String,
}

/// Capability check sub-span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityCheckSpan {
    pub capability: String,
    pub result: CheckResult,
}

/// Fuel check sub-span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelCheckSpan {
    pub fuel_required: u64,
    pub fuel_remaining: u64,
    pub result: CheckResult,
}

/// HITL approval gate sub-span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitlGateSpan {
    pub reason: String,
    pub decision: HitlDecision,
    pub response_time_ms: u64,
}

/// WASM sandbox execution sub-span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxSpan {
    pub wasm_fuel_consumed: u64,
    pub memory_peak_bytes: u64,
    pub duration_ms: u64,
}

/// LLM request sub-span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequestSpan {
    pub provider: String,
    pub model: String,
    pub tokens_input: u64,
    pub tokens_output: u64,
    pub latency_ms: u64,
}

/// PII redaction sub-span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiiRedactionSpan {
    pub items_detected: u64,
    pub items_redacted: u64,
}

/// Audit write sub-span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditWriteSpan {
    pub entry_id: String,
    pub chain_length: u64,
}

/// Emit tracing events for an agent execution span.
///
/// This uses the `tracing` crate macros so the spans integrate with any
/// `tracing-subscriber` backend (JSON logs, OTLP exporter, etc.).
pub fn emit_agent_execution(span: &AgentExecutionSpan) {
    tracing::info_span!("nexus.agent.execute",
        agent_did = %span.agent_did,
        autonomy_level = span.autonomy_level,
        task_type = %span.task_type,
        duration_ms = span.duration_ms,
        status = %span.status,
    )
    .in_scope(|| {
        for cap in &span.capability_checks {
            tracing::info!(
                capability = %cap.capability,
                result = %cap.result,
                "nexus.capability.check"
            );
        }

        if let Some(ref fuel) = span.fuel_check {
            tracing::info!(
                fuel_required = fuel.fuel_required,
                fuel_remaining = fuel.fuel_remaining,
                result = %fuel.result,
                "nexus.fuel.check"
            );
        }

        if let Some(ref hitl) = span.hitl_gate {
            tracing::info!(
                reason = %hitl.reason,
                decision = %hitl.decision,
                response_time_ms = hitl.response_time_ms,
                "nexus.hitl.gate"
            );
        }

        if let Some(ref sandbox) = span.sandbox_execution {
            tracing::info!(
                wasm_fuel_consumed = sandbox.wasm_fuel_consumed,
                memory_peak_bytes = sandbox.memory_peak_bytes,
                duration_ms = sandbox.duration_ms,
                "nexus.sandbox.execute"
            );
        }

        if let Some(ref llm) = span.llm_request {
            tracing::info!(
                provider = %llm.provider,
                model = %llm.model,
                tokens_input = llm.tokens_input,
                tokens_output = llm.tokens_output,
                latency_ms = llm.latency_ms,
                "nexus.llm.request"
            );
        }

        if let Some(ref pii) = span.pii_redaction {
            tracing::info!(
                items_detected = pii.items_detected,
                items_redacted = pii.items_redacted,
                "nexus.pii.redaction"
            );
        }

        if let Some(ref audit) = span.audit_write {
            tracing::info!(
                entry_id = %audit.entry_id,
                chain_length = audit.chain_length,
                "nexus.audit.write"
            );
        }
    });
}

/// Convenience: emit an LLM request span standalone (for the LLM router).
pub fn emit_llm_request(span: &LlmRequestSpan) {
    tracing::info!(
        provider = %span.provider,
        model = %span.model,
        tokens_input = span.tokens_input,
        tokens_output = span.tokens_output,
        latency_ms = span.latency_ms,
        "nexus.llm.request"
    );
}

/// Convert span attributes to a flat HashMap for serialization/export.
pub fn span_to_attributes(span: &AgentExecutionSpan) -> HashMap<String, serde_json::Value> {
    let mut attrs = HashMap::new();
    attrs.insert(
        "agent_did".to_string(),
        serde_json::Value::String(span.agent_did.clone()),
    );
    attrs.insert(
        "autonomy_level".to_string(),
        serde_json::json!(span.autonomy_level),
    );
    attrs.insert(
        "task_type".to_string(),
        serde_json::Value::String(span.task_type.clone()),
    );
    attrs.insert(
        "duration_ms".to_string(),
        serde_json::json!(span.duration_ms),
    );
    attrs.insert(
        "status".to_string(),
        serde_json::Value::String(span.status.clone()),
    );
    attrs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_result_display() {
        assert_eq!(format!("{}", CheckResult::Granted), "granted");
        assert_eq!(format!("{}", CheckResult::Exhausted), "exhausted");
    }

    #[test]
    fn hitl_decision_display() {
        assert_eq!(format!("{}", HitlDecision::Approved), "approved");
        assert_eq!(format!("{}", HitlDecision::Timeout), "timeout");
    }

    #[test]
    fn agent_execution_span_serde() {
        let span = AgentExecutionSpan {
            agent_did: "did:key:z6MkTest".to_string(),
            autonomy_level: 2,
            task_type: "code_generation".to_string(),
            capability_checks: vec![CapabilityCheckSpan {
                capability: "llm.invoke".to_string(),
                result: CheckResult::Granted,
            }],
            fuel_check: Some(FuelCheckSpan {
                fuel_required: 100,
                fuel_remaining: 500,
                result: CheckResult::Sufficient,
            }),
            hitl_gate: None,
            sandbox_execution: Some(SandboxSpan {
                wasm_fuel_consumed: 80,
                memory_peak_bytes: 1024 * 1024,
                duration_ms: 42,
            }),
            llm_request: Some(LlmRequestSpan {
                provider: "claude".to_string(),
                model: "claude-sonnet-4-6".to_string(),
                tokens_input: 500,
                tokens_output: 200,
                latency_ms: 1200,
            }),
            pii_redaction: Some(PiiRedactionSpan {
                items_detected: 3,
                items_redacted: 3,
            }),
            audit_write: Some(AuditWriteSpan {
                entry_id: "evt-001".to_string(),
                chain_length: 42,
            }),
            duration_ms: 1500,
            status: "ok".to_string(),
        };

        let json = serde_json::to_string(&span).unwrap();
        let parsed: AgentExecutionSpan = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.agent_did, "did:key:z6MkTest");
        assert_eq!(parsed.capability_checks.len(), 1);
        assert!(parsed.llm_request.is_some());
    }

    #[test]
    fn span_to_attributes_extracts_fields() {
        let span = AgentExecutionSpan {
            agent_did: "did:key:z6MkTest".to_string(),
            autonomy_level: 3,
            task_type: "deploy".to_string(),
            capability_checks: vec![],
            fuel_check: None,
            hitl_gate: None,
            sandbox_execution: None,
            llm_request: None,
            pii_redaction: None,
            audit_write: None,
            duration_ms: 100,
            status: "ok".to_string(),
        };

        let attrs = span_to_attributes(&span);
        assert_eq!(attrs["agent_did"], serde_json::json!("did:key:z6MkTest"));
        assert_eq!(attrs["autonomy_level"], serde_json::json!(3));
    }

    #[test]
    fn emit_agent_execution_does_not_panic() {
        // Verify that emit_agent_execution doesn't panic even without a subscriber.
        let span = AgentExecutionSpan {
            agent_did: "did:key:test".to_string(),
            autonomy_level: 1,
            task_type: "test".to_string(),
            capability_checks: vec![
                CapabilityCheckSpan {
                    capability: "net.http".to_string(),
                    result: CheckResult::Granted,
                },
                CapabilityCheckSpan {
                    capability: "fs.write".to_string(),
                    result: CheckResult::Denied,
                },
            ],
            fuel_check: Some(FuelCheckSpan {
                fuel_required: 50,
                fuel_remaining: 100,
                result: CheckResult::Sufficient,
            }),
            hitl_gate: Some(HitlGateSpan {
                reason: "Tier2 operation".to_string(),
                decision: HitlDecision::Approved,
                response_time_ms: 3200,
            }),
            sandbox_execution: None,
            llm_request: None,
            pii_redaction: None,
            audit_write: None,
            duration_ms: 50,
            status: "ok".to_string(),
        };
        emit_agent_execution(&span);
    }
}
