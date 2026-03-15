//! EU AI Act Article 13 transparency report generation.
//!
//! Produces structured (JSON) and human-readable (Markdown) transparency
//! reports from existing `AuditTrail` events and `AgentManifest` data.

use crate::audit::{AuditTrail, EventType};
use crate::autonomy::AutonomyLevel;
use crate::compliance::eu_ai_act::RiskClassifier;
use crate::manifest::AgentManifest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Capability risk level label — mirrors permissions.rs but kept simple here
/// so the transparency module has no coupling to the dashboard internals.
fn capability_risk_label(cap: &str) -> &'static str {
    match cap {
        "audit.read" => "low",
        "fs.read" | "web.search" | "web.read" | "llm.query" => "medium",
        "fs.write" | "social.post" | "social.x.post" | "messaging.send" => "high",
        "process.exec" => "critical",
        "social.x.read" => "low",
        _ => "unknown",
    }
}

/// Plain-English description of each autonomy level.
fn autonomy_plain_english(level: AutonomyLevel) -> &'static str {
    match level {
        AutonomyLevel::L0 => "Inert — the agent cannot act on its own",
        AutonomyLevel::L1 => "Suggest — the agent can suggest actions but a human decides",
        AutonomyLevel::L2 => "Act-with-approval — the agent acts only after human approval",
        AutonomyLevel::L3 => "Act-then-report — the agent acts first and reports to a human after",
        AutonomyLevel::L4 => {
            "Autonomous-bounded — the agent acts autonomously within limits, escalating anomalies"
        }
        AutonomyLevel::L5 => {
            "Full autonomy — the agent operates independently, only the kernel can override"
        }
        AutonomyLevel::L6 => {
            "Transcendent autonomy — the agent can adapt its cognition, coordinate multiple models, and design governed ecosystems"
        }
    }
}

// ---------------------------------------------------------------------------
// Report data structures
// ---------------------------------------------------------------------------

/// A single capability with its risk assessment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityEntry {
    pub capability: String,
    pub risk_level: String,
}

/// Summary of data processing derived from audit events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataProcessingSummary {
    pub total_events: usize,
    pub llm_calls: usize,
    pub tool_calls: usize,
    pub state_changes: usize,
    pub errors: usize,
    pub user_actions: usize,
    /// Distinct event payload keys observed (approximation of data types).
    pub data_types_observed: Vec<String>,
}

/// Summary of human oversight interactions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanOversightSummary {
    pub total_approvals: usize,
    pub total_rejections: usize,
    pub approval_rate_percent: u8,
    pub rejection_reasons: Vec<String>,
}

/// Model and provider information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub configured_model: Option<String>,
    pub provider_type: String,
}

/// Resource consumption summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub fuel_budget: u64,
    pub fuel_consumed_events: usize,
    pub audit_events_generated: usize,
}

/// Full transparency report per EU AI Act Article 13.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransparencyReport {
    pub report_version: String,
    pub agent_name: String,
    pub agent_did: Option<String>,
    pub risk_tier: String,
    pub risk_justification: String,
    pub applicable_articles: Vec<String>,
    pub required_controls: Vec<String>,
    pub capabilities: Vec<CapabilityEntry>,
    pub autonomy_level: String,
    pub autonomy_description: String,
    pub data_processing: DataProcessingSummary,
    pub human_oversight: HumanOversightSummary,
    pub model_info: ModelInfo,
    pub resource_usage: ResourceUsage,
    pub generated_at_unix: u64,
}

impl TransparencyReport {
    /// Serialize to JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Render as human-readable Markdown.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str(&format!("# Transparency Report: {}\n\n", self.agent_name));

        if let Some(did) = &self.agent_did {
            md.push_str(&format!("**Agent DID:** `{}`\n\n", did));
        }

        // Risk classification
        md.push_str("## Risk Classification\n\n");
        md.push_str(&format!("- **Tier:** {}\n", self.risk_tier));
        md.push_str(&format!(
            "- **Justification:** {}\n",
            self.risk_justification
        ));
        if !self.applicable_articles.is_empty() {
            md.push_str("- **Applicable articles:**\n");
            for article in &self.applicable_articles {
                md.push_str(&format!("  - {}\n", article));
            }
        }
        if !self.required_controls.is_empty() {
            md.push_str("- **Required controls:**\n");
            for control in &self.required_controls {
                md.push_str(&format!("  - {}\n", control));
            }
        }
        md.push('\n');

        // Capabilities
        md.push_str("## Granted Capabilities\n\n");
        md.push_str("| Capability | Risk Level |\n");
        md.push_str("|------------|------------|\n");
        for cap in &self.capabilities {
            md.push_str(&format!("| {} | {} |\n", cap.capability, cap.risk_level));
        }
        md.push('\n');

        // Autonomy
        md.push_str("## Autonomy Level\n\n");
        md.push_str(&format!("- **Level:** {}\n", self.autonomy_level));
        md.push_str(&format!("- **Meaning:** {}\n\n", self.autonomy_description));

        // Data processing
        md.push_str("## Data Processing Summary\n\n");
        md.push_str(&format!(
            "- **Total audit events:** {}\n",
            self.data_processing.total_events
        ));
        md.push_str(&format!(
            "- **LLM calls:** {}\n",
            self.data_processing.llm_calls
        ));
        md.push_str(&format!(
            "- **Tool calls:** {}\n",
            self.data_processing.tool_calls
        ));
        md.push_str(&format!(
            "- **State changes:** {}\n",
            self.data_processing.state_changes
        ));
        md.push_str(&format!("- **Errors:** {}\n", self.data_processing.errors));
        md.push_str(&format!(
            "- **User actions:** {}\n",
            self.data_processing.user_actions
        ));
        if !self.data_processing.data_types_observed.is_empty() {
            md.push_str("- **Data types observed:**\n");
            for dt in &self.data_processing.data_types_observed {
                md.push_str(&format!("  - {}\n", dt));
            }
        }
        md.push('\n');

        // Human oversight
        md.push_str("## Human Oversight\n\n");
        md.push_str(&format!(
            "- **Approvals:** {}\n",
            self.human_oversight.total_approvals
        ));
        md.push_str(&format!(
            "- **Rejections:** {}\n",
            self.human_oversight.total_rejections
        ));
        md.push_str(&format!(
            "- **Approval rate:** {}%\n",
            self.human_oversight.approval_rate_percent
        ));
        if !self.human_oversight.rejection_reasons.is_empty() {
            md.push_str("- **Rejection reasons:**\n");
            for reason in &self.human_oversight.rejection_reasons {
                md.push_str(&format!("  - {}\n", reason));
            }
        }
        md.push('\n');

        // Model info
        md.push_str("## Model Information\n\n");
        md.push_str(&format!(
            "- **Configured model:** {}\n",
            self.model_info
                .configured_model
                .as_deref()
                .unwrap_or("none")
        ));
        md.push_str(&format!(
            "- **Provider type:** {}\n\n",
            self.model_info.provider_type
        ));

        // Resource usage
        md.push_str("## Resource Usage\n\n");
        md.push_str(&format!(
            "- **Fuel budget:** {}\n",
            self.resource_usage.fuel_budget
        ));
        md.push_str(&format!(
            "- **Fuel consumption events:** {}\n",
            self.resource_usage.fuel_consumed_events
        ));
        md.push_str(&format!(
            "- **Audit events generated:** {}\n",
            self.resource_usage.audit_events_generated
        ));

        md
    }
}

// ---------------------------------------------------------------------------
// Generator
// ---------------------------------------------------------------------------

/// Generates EU AI Act Article 13 transparency reports from audit data.
#[derive(Debug, Clone, Default)]
pub struct TransparencyReportGenerator {
    classifier: RiskClassifier,
}

impl TransparencyReportGenerator {
    pub fn new() -> Self {
        Self {
            classifier: RiskClassifier::new(),
        }
    }

    /// Generate a transparency report for an agent.
    ///
    /// * `manifest` — the agent's manifest (capabilities, autonomy, model, etc.)
    /// * `agent_did` — optional DID string from the agent identity system
    /// * `audit_trail` — the audit trail to analyse for data processing stats
    /// * `agent_id` — the agent's UUID (used to filter audit events)
    pub fn generate(
        &self,
        manifest: &AgentManifest,
        agent_did: Option<&str>,
        audit_trail: &AuditTrail,
        agent_id: uuid::Uuid,
    ) -> TransparencyReport {
        let profile = self.classifier.classify_agent(manifest);
        let autonomy = AutonomyLevel::from_manifest(manifest.autonomy_level);

        let capabilities: Vec<CapabilityEntry> = manifest
            .capabilities
            .iter()
            .map(|c| CapabilityEntry {
                capability: c.clone(),
                risk_level: capability_risk_label(c).to_string(),
            })
            .collect();

        let data_processing = self.analyse_data_processing(audit_trail, agent_id);
        let human_oversight = self.analyse_human_oversight(audit_trail, agent_id);
        let resource_usage = self.analyse_resource_usage(manifest, audit_trail, agent_id);
        let model_info = self.extract_model_info(manifest);

        let generated_at_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        TransparencyReport {
            report_version: "1.0.0".to_string(),
            agent_name: manifest.name.clone(),
            agent_did: agent_did.map(String::from),
            risk_tier: profile.tier.as_str().to_string(),
            risk_justification: profile.justification,
            applicable_articles: profile.applicable_articles,
            required_controls: profile.required_controls,
            capabilities,
            autonomy_level: autonomy.as_str().to_string(),
            autonomy_description: autonomy_plain_english(autonomy).to_string(),
            data_processing,
            human_oversight,
            model_info,
            resource_usage,
            generated_at_unix,
        }
    }

    fn analyse_data_processing(
        &self,
        audit_trail: &AuditTrail,
        agent_id: uuid::Uuid,
    ) -> DataProcessingSummary {
        let events = audit_trail.events();
        let agent_events: Vec<_> = events.iter().filter(|e| e.agent_id == agent_id).collect();

        let mut llm_calls = 0usize;
        let mut tool_calls = 0usize;
        let mut state_changes = 0usize;
        let mut errors = 0usize;
        let mut user_actions = 0usize;
        let mut data_types: BTreeMap<String, ()> = BTreeMap::new();

        for event in &agent_events {
            match event.event_type {
                EventType::LlmCall => llm_calls += 1,
                EventType::ToolCall => tool_calls += 1,
                EventType::StateChange => state_changes += 1,
                EventType::Error => errors += 1,
                EventType::UserAction => user_actions += 1,
            }

            // Extract top-level payload keys as approximate data types.
            if let Some(obj) = event.payload.as_object() {
                for key in obj.keys() {
                    data_types.insert(key.clone(), ());
                }
            }
        }

        DataProcessingSummary {
            total_events: agent_events.len(),
            llm_calls,
            tool_calls,
            state_changes,
            errors,
            user_actions,
            data_types_observed: data_types.into_keys().collect(),
        }
    }

    fn analyse_human_oversight(
        &self,
        audit_trail: &AuditTrail,
        agent_id: uuid::Uuid,
    ) -> HumanOversightSummary {
        let events = audit_trail.events();
        let mut approvals = 0usize;
        let mut rejections = 0usize;
        let mut rejection_reasons: Vec<String> = Vec::new();

        for event in events.iter().filter(|e| e.agent_id == agent_id) {
            if let Some(obj) = event.payload.as_object() {
                // Look for approval/consent events by checking payload keys.
                if let Some(verdict) = obj
                    .get("verdict")
                    .or_else(|| obj.get("decision"))
                    .and_then(|v| v.as_str())
                {
                    match verdict {
                        "approved" | "approve" | "granted" => approvals += 1,
                        "denied" | "deny" | "rejected" => {
                            rejections += 1;
                            if let Some(reason) = obj.get("reason").and_then(|v| v.as_str()) {
                                let reason_str = reason.to_string();
                                if !rejection_reasons.contains(&reason_str) {
                                    rejection_reasons.push(reason_str);
                                }
                            }
                        }
                        _ => {}
                    }
                }

                // Also check for "approval" / "consent" event markers.
                if let Some(event_name) = obj.get("event").and_then(|v| v.as_str()) {
                    if event_name.contains("approved") || event_name.contains("consent.granted") {
                        approvals += 1;
                    } else if event_name.contains("denied") || event_name.contains("rejected") {
                        rejections += 1;
                    }
                }
            }
        }

        let total = approvals + rejections;
        let approval_rate_percent = if total > 0 {
            ((approvals * 100) / total) as u8
        } else {
            100 // No decisions needed = fully compliant
        };

        HumanOversightSummary {
            total_approvals: approvals,
            total_rejections: rejections,
            approval_rate_percent,
            rejection_reasons,
        }
    }

    fn analyse_resource_usage(
        &self,
        manifest: &AgentManifest,
        audit_trail: &AuditTrail,
        agent_id: uuid::Uuid,
    ) -> ResourceUsage {
        let events = audit_trail.events();
        let agent_events = events.iter().filter(|e| e.agent_id == agent_id);

        let fuel_consumed_events = agent_events
            .filter(|e| {
                e.payload
                    .as_object()
                    .is_some_and(|obj| obj.contains_key("fuel") || obj.contains_key("fuel_cost"))
            })
            .count();

        let audit_events_generated = events.iter().filter(|e| e.agent_id == agent_id).count();

        ResourceUsage {
            fuel_budget: manifest.fuel_budget,
            fuel_consumed_events,
            audit_events_generated,
        }
    }

    fn extract_model_info(&self, manifest: &AgentManifest) -> ModelInfo {
        let configured_model = manifest.llm_model.clone();

        // Determine provider type from model name heuristics.
        let provider_type = match configured_model.as_deref() {
            Some(m) if m.contains("claude") => "cloud (Anthropic)",
            Some(m) if m.contains("gpt") => "cloud (OpenAI)",
            Some(m) if m.contains("local") || m.contains("slm") || m.contains("llama") => {
                "local SLM"
            }
            Some(_) => "cloud (unknown provider)",
            None => "none configured",
        };

        ModelInfo {
            configured_model,
            provider_type: provider_type.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditTrail, EventType};
    use crate::manifest::AgentManifest;
    use serde_json::json;
    use uuid::Uuid;

    fn base_manifest(name: &str, caps: Vec<&str>) -> AgentManifest {
        AgentManifest {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            capabilities: caps.into_iter().map(String::from).collect(),
            fuel_budget: 5000,
            autonomy_level: None,
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            default_goal: None,
            llm_model: Some("claude-sonnet-4-5".to_string()),
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
            filesystem_permissions: vec![],
        }
    }

    fn trail_with_events(agent_id: Uuid) -> AuditTrail {
        let mut trail = AuditTrail::new();

        // LLM calls
        for i in 0..3 {
            trail
                .append_event(
                    agent_id,
                    EventType::LlmCall,
                    json!({"prompt": format!("query {}", i), "model": "claude-sonnet-4-5"}),
                )
                .unwrap();
        }

        // Tool calls
        trail
            .append_event(
                agent_id,
                EventType::ToolCall,
                json!({"tool": "web.search", "query": "rust async"}),
            )
            .unwrap();

        // State change
        trail
            .append_event(
                agent_id,
                EventType::StateChange,
                json!({"event": "agent.started"}),
            )
            .unwrap();

        // Approval events
        trail
            .append_event(
                agent_id,
                EventType::UserAction,
                json!({"verdict": "approved", "operation": "tool_call"}),
            )
            .unwrap();
        trail
            .append_event(
                agent_id,
                EventType::UserAction,
                json!({"verdict": "denied", "reason": "too risky", "operation": "fs.write"}),
            )
            .unwrap();

        // Fuel event
        trail
            .append_event(
                agent_id,
                EventType::StateChange,
                json!({"fuel": 42, "reason": "llm_call"}),
            )
            .unwrap();

        // Error
        trail
            .append_event(
                agent_id,
                EventType::Error,
                json!({"error": "timeout", "code": 504}),
            )
            .unwrap();

        trail
    }

    #[test]
    fn report_contains_all_required_fields() {
        let agent_id = Uuid::new_v4();
        let manifest = base_manifest("test-agent", vec!["llm.query", "fs.read"]);
        let trail = trail_with_events(agent_id);
        let did = "did:key:z6MkTest1234";

        let gen = TransparencyReportGenerator::new();
        let report = gen.generate(&manifest, Some(did), &trail, agent_id);

        // Identity
        assert_eq!(report.agent_name, "test-agent");
        assert_eq!(report.agent_did, Some(did.to_string()));
        assert_eq!(report.report_version, "1.0.0");

        // Risk classification
        assert!(!report.risk_tier.is_empty());
        assert!(!report.risk_justification.is_empty());

        // Capabilities
        assert_eq!(report.capabilities.len(), 2);
        assert!(report
            .capabilities
            .iter()
            .any(|c| c.capability == "llm.query"));
        assert!(report
            .capabilities
            .iter()
            .any(|c| c.capability == "fs.read"));

        // Autonomy
        assert_eq!(report.autonomy_level, "L0");
        assert!(!report.autonomy_description.is_empty());

        // Data processing
        assert!(report.data_processing.total_events > 0);
        assert_eq!(report.data_processing.llm_calls, 3);
        assert_eq!(report.data_processing.tool_calls, 1);
        assert!(report.data_processing.state_changes >= 1);
        assert_eq!(report.data_processing.errors, 1);

        // Human oversight
        assert_eq!(report.human_oversight.total_approvals, 1);
        assert_eq!(report.human_oversight.total_rejections, 1);
        assert_eq!(report.human_oversight.approval_rate_percent, 50);
        assert!(report
            .human_oversight
            .rejection_reasons
            .contains(&"too risky".to_string()));

        // Model info
        assert_eq!(
            report.model_info.configured_model,
            Some("claude-sonnet-4-5".to_string())
        );
        assert_eq!(report.model_info.provider_type, "cloud (Anthropic)");

        // Resource usage
        assert_eq!(report.resource_usage.fuel_budget, 5000);
        assert!(report.resource_usage.audit_events_generated > 0);

        // Timestamp
        assert!(report.generated_at_unix > 0);
    }

    #[test]
    fn high_risk_agent_includes_oversight_section() {
        let agent_id = Uuid::new_v4();
        let manifest = base_manifest("risky-agent", vec!["fs.write", "web.search", "llm.query"]);
        let trail = trail_with_events(agent_id);

        let gen = TransparencyReportGenerator::new();
        let report = gen.generate(&manifest, None, &trail, agent_id);

        assert_eq!(report.risk_tier, "high");
        assert!(!report.applicable_articles.is_empty());
        assert!(report
            .applicable_articles
            .iter()
            .any(|a| a.contains("Article 14")));
        assert!(!report.required_controls.is_empty());
        assert!(report
            .required_controls
            .iter()
            .any(|c| c.contains("Human oversight")));

        // Oversight section populated
        assert!(
            report.human_oversight.total_approvals > 0
                || report.human_oversight.total_rejections > 0
        );
    }

    #[test]
    fn markdown_output_is_human_readable() {
        let agent_id = Uuid::new_v4();
        let mut manifest = base_manifest("my-llm-agent", vec!["llm.query", "fs.read"]);
        manifest.autonomy_level = Some(2);

        let trail = trail_with_events(agent_id);
        let did = "did:key:z6MkExample";

        let gen = TransparencyReportGenerator::new();
        let report = gen.generate(&manifest, Some(did), &trail, agent_id);
        let md = report.to_markdown();

        // Title
        assert!(md.contains("# Transparency Report: my-llm-agent"));

        // DID
        assert!(md.contains("did:key:z6MkExample"));

        // Risk section
        assert!(md.contains("## Risk Classification"));
        assert!(md.contains("**Tier:**"));

        // Capabilities table
        assert!(md.contains("## Granted Capabilities"));
        assert!(md.contains("| llm.query |"));

        // Autonomy section
        assert!(md.contains("## Autonomy Level"));
        assert!(md.contains("L2"));
        assert!(md.contains("Act-with-approval"));

        // Data processing
        assert!(md.contains("## Data Processing Summary"));
        assert!(md.contains("**LLM calls:**"));

        // Human oversight
        assert!(md.contains("## Human Oversight"));
        assert!(md.contains("**Approvals:**"));

        // Model info
        assert!(md.contains("## Model Information"));
        assert!(md.contains("claude-sonnet-4-5"));

        // Resource usage
        assert!(md.contains("## Resource Usage"));
        assert!(md.contains("**Fuel budget:**"));
    }

    #[test]
    fn json_output_is_valid() {
        let agent_id = Uuid::new_v4();
        let manifest = base_manifest("json-agent", vec!["audit.read"]);
        let trail = AuditTrail::new();

        let gen = TransparencyReportGenerator::new();
        let report = gen.generate(&manifest, None, &trail, agent_id);
        let json_str = report.to_json();

        let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
        assert!(parsed.is_object());
        assert!(parsed.get("agent_name").is_some());
        assert!(parsed.get("risk_tier").is_some());
        assert!(parsed.get("capabilities").is_some());
        assert!(parsed.get("human_oversight").is_some());
    }

    #[test]
    fn empty_audit_trail_produces_valid_report() {
        let agent_id = Uuid::new_v4();
        let manifest = base_manifest("idle-agent", vec!["fs.read"]);
        let trail = AuditTrail::new();

        let gen = TransparencyReportGenerator::new();
        let report = gen.generate(&manifest, None, &trail, agent_id);

        assert_eq!(report.data_processing.total_events, 0);
        assert_eq!(report.data_processing.llm_calls, 0);
        assert_eq!(report.human_oversight.total_approvals, 0);
        assert_eq!(report.human_oversight.approval_rate_percent, 100);
        assert_eq!(report.resource_usage.audit_events_generated, 0);
    }

    #[test]
    fn filters_events_by_agent_id() {
        let agent_a = Uuid::new_v4();
        let agent_b = Uuid::new_v4();
        let mut trail = AuditTrail::new();

        // Events for agent A
        for _ in 0..5 {
            trail
                .append_event(agent_a, EventType::LlmCall, json!({"model": "test"}))
                .unwrap();
        }
        // Events for agent B
        for _ in 0..3 {
            trail
                .append_event(agent_b, EventType::ToolCall, json!({"tool": "test"}))
                .unwrap();
        }

        let manifest = base_manifest("agent-a", vec!["llm.query"]);
        let gen = TransparencyReportGenerator::new();
        let report = gen.generate(&manifest, None, &trail, agent_a);

        assert_eq!(report.data_processing.total_events, 5);
        assert_eq!(report.data_processing.llm_calls, 5);
        assert_eq!(report.data_processing.tool_calls, 0);
    }

    #[test]
    fn local_model_detected() {
        let agent_id = Uuid::new_v4();
        let mut manifest = base_manifest("local-agent", vec!["llm.query"]);
        manifest.llm_model = Some("local-llama-7b".to_string());

        let gen = TransparencyReportGenerator::new();
        let report = gen.generate(&manifest, None, &AuditTrail::new(), agent_id);

        assert_eq!(report.model_info.provider_type, "local SLM");
    }

    #[test]
    fn no_model_configured() {
        let agent_id = Uuid::new_v4();
        let mut manifest = base_manifest("no-model", vec!["fs.read"]);
        manifest.llm_model = None;

        let gen = TransparencyReportGenerator::new();
        let report = gen.generate(&manifest, None, &AuditTrail::new(), agent_id);

        assert_eq!(report.model_info.configured_model, None);
        assert_eq!(report.model_info.provider_type, "none configured");
    }

    #[test]
    fn data_types_observed_extracted_from_payloads() {
        let agent_id = Uuid::new_v4();
        let mut trail = AuditTrail::new();
        trail
            .append_event(
                agent_id,
                EventType::LlmCall,
                json!({"prompt": "hello", "model": "test", "tokens": 42}),
            )
            .unwrap();
        trail
            .append_event(
                agent_id,
                EventType::ToolCall,
                json!({"tool": "fs.read", "path": "/tmp/test"}),
            )
            .unwrap();

        let manifest = base_manifest("data-agent", vec!["llm.query"]);
        let gen = TransparencyReportGenerator::new();
        let report = gen.generate(&manifest, None, &trail, agent_id);

        let types = &report.data_processing.data_types_observed;
        assert!(types.contains(&"prompt".to_string()));
        assert!(types.contains(&"model".to_string()));
        assert!(types.contains(&"tool".to_string()));
        assert!(types.contains(&"path".to_string()));
    }
}
