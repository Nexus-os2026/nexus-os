use nexus_connectors_llm::providers::LlmProvider;
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrashContext {
    pub agent_id: Uuid,
    pub crash_reason: String,
    pub event_log_excerpt: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BugReport {
    pub report_id: String,
    pub agent_id: String,
    pub severity: String,
    pub root_cause: String,
    pub steps_to_reproduce: Vec<String>,
    pub expected_behavior: String,
    pub actual_behavior: String,
    pub audit_trail_excerpt: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LlmAnalysis {
    root_cause: Option<String>,
    steps_to_reproduce: Option<Vec<String>>,
    severity: Option<String>,
    expected_behavior: Option<String>,
    actual_behavior: Option<String>,
}

pub struct ErrorAnalyzer<P: LlmProvider> {
    provider: P,
    model_name: String,
    max_tokens: u32,
}

impl<P: LlmProvider> ErrorAnalyzer<P> {
    pub fn new(provider: P, model_name: &str) -> Self {
        Self {
            provider,
            model_name: model_name.to_string(),
            max_tokens: 256,
        }
    }

    pub fn capture_crash_context(
        &self,
        agent_id: Uuid,
        crash_reason: &str,
        audit_trail: &AuditTrail,
    ) -> CrashContext {
        let mut related = audit_trail
            .events()
            .iter()
            .filter(|event| event.agent_id == agent_id)
            .map(summarize_event)
            .collect::<Vec<_>>();
        if related.is_empty() {
            related = audit_trail
                .events()
                .iter()
                .rev()
                .take(6)
                .map(summarize_event)
                .collect::<Vec<_>>();
            related.reverse();
        }

        CrashContext {
            agent_id,
            crash_reason: crash_reason.to_string(),
            event_log_excerpt: related.into_iter().rev().take(8).rev().collect(),
        }
    }

    pub fn analyze_crash(
        &self,
        agent_id: Uuid,
        crash_reason: &str,
        audit_trail: &AuditTrail,
    ) -> Result<BugReport, AgentError> {
        let context = self.capture_crash_context(agent_id, crash_reason, audit_trail);
        let prompt = format!(
            "Analyze this crash and return JSON keys root_cause, steps_to_reproduce (array), severity, expected_behavior, actual_behavior.\nCrash: {}\nEvents:\n{}",
            context.crash_reason,
            context.event_log_excerpt.join("\n")
        );
        let llm_result = self
            .provider
            .query(prompt.as_str(), self.max_tokens, self.model_name.as_str())
            .ok()
            .and_then(|response| {
                serde_json::from_str::<LlmAnalysis>(response.output_text.as_str()).ok()
            });

        let fallback = heuristic_analysis(&context);
        let analysis = llm_result.unwrap_or(fallback);

        Ok(BugReport {
            report_id: format!("bug-{}", Uuid::new_v4()),
            agent_id: context.agent_id.to_string(),
            severity: normalize_severity(
                analysis
                    .severity
                    .as_deref()
                    .unwrap_or("medium")
                    .to_string()
                    .as_str(),
            ),
            root_cause: analysis
                .root_cause
                .unwrap_or_else(|| "Unknown crash cause; inspect audit trail".to_string()),
            steps_to_reproduce: analysis.steps_to_reproduce.unwrap_or_else(|| {
                vec![
                    "Start agent with same manifest and capabilities".to_string(),
                    "Replay audit trail sequence around the crash".to_string(),
                    "Observe runtime failure during the same stage".to_string(),
                ]
            }),
            expected_behavior: analysis
                .expected_behavior
                .unwrap_or_else(|| "Agent continues running without fatal errors".to_string()),
            actual_behavior: analysis
                .actual_behavior
                .unwrap_or_else(|| format!("Agent crashed: {}", context.crash_reason)),
            audit_trail_excerpt: context.event_log_excerpt,
        })
    }
}

fn summarize_event(event: &nexus_kernel::audit::AuditEvent) -> String {
    format!(
        "[{}] {} {}",
        event.timestamp,
        event_type_label(&event.event_type),
        event.payload
    )
}

fn event_type_label(event_type: &EventType) -> &'static str {
    match event_type {
        EventType::StateChange => "state_change",
        EventType::ToolCall => "tool_call",
        EventType::LlmCall => "llm_call",
        EventType::Error => "error",
        EventType::UserAction => "user_action",
    }
}

fn normalize_severity(input: &str) -> String {
    let normalized = input.trim().to_lowercase();
    match normalized.as_str() {
        "low" | "medium" | "high" | "critical" => normalized,
        _ => "medium".to_string(),
    }
}

fn heuristic_analysis(context: &CrashContext) -> LlmAnalysis {
    let joined = context
        .event_log_excerpt
        .iter()
        .map(|line| line.to_lowercase())
        .collect::<Vec<_>>()
        .join("\n");
    let reason = context.crash_reason.to_lowercase();

    let (root_cause, severity) = if reason.contains("fuel") || joined.contains("fuelexhausted") {
        (
            "Fuel budget exhausted before completing workflow".to_string(),
            "medium".to_string(),
        )
    } else if reason.contains("capability") || joined.contains("capability denied") {
        (
            "Missing required capability for attempted operation".to_string(),
            "high".to_string(),
        )
    } else if reason.contains("panic") || joined.contains("panic") {
        (
            "Unhandled runtime panic".to_string(),
            "critical".to_string(),
        )
    } else {
        (
            "Unhandled runtime error triggered by event sequence".to_string(),
            "high".to_string(),
        )
    };

    LlmAnalysis {
        root_cause: Some(root_cause),
        steps_to_reproduce: Some(vec![
            "Launch the agent with the same configuration".to_string(),
            "Execute the same sequence of actions from the audit trail excerpt".to_string(),
            "Observe the crash at the same failure point".to_string(),
        ]),
        severity: Some(severity),
        expected_behavior: Some("Agent should handle the failing condition gracefully".to_string()),
        actual_behavior: Some(format!(
            "Agent crashed with reason: {}",
            context.crash_reason
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::ErrorAnalyzer;
    use nexus_connectors_llm::providers::{LlmProvider, LlmResponse};
    use nexus_kernel::audit::{AuditTrail, EventType};
    use nexus_kernel::errors::AgentError;
    use serde_json::json;
    use uuid::Uuid;

    struct MockCrashAnalysisProvider;

    impl LlmProvider for MockCrashAnalysisProvider {
        fn query(
            &self,
            _prompt: &str,
            max_tokens: u32,
            model: &str,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                output_text: r#"{
                    "root_cause": "CapabilityDenied when calling external tool",
                    "steps_to_reproduce": [
                        "Create agent without missing capability",
                        "Run workflow that invokes the tool",
                        "Observe crash in runtime logs"
                    ],
                    "severity": "high",
                    "expected_behavior": "Agent reports denied action and continues safely",
                    "actual_behavior": "Agent terminated after denied tool call"
                }"#
                .to_string(),
                token_count: max_tokens.min(64),
                model_name: model.to_string(),
                tool_calls: Vec::new(),
            })
        }

        fn name(&self) -> &str {
            "mock-crash-analysis"
        }

        fn cost_per_token(&self) -> f64 {
            0.0
        }
    }

    #[test]
    fn test_bug_report_generation() {
        let analyzer = ErrorAnalyzer::new(MockCrashAnalysisProvider, "mock-model");
        let agent_id = Uuid::new_v4();
        let mut trail = AuditTrail::new();
        trail
            .append_event(
                agent_id,
                EventType::StateChange,
                json!({"state": "running"}),
            )
            .expect("audit append");
        trail
            .append_event(
                agent_id,
                EventType::Error,
                json!({"error": "CapabilityDenied", "capability": "web.search"}),
            )
            .expect("audit append");

        let report = analyzer
            .analyze_crash(agent_id, "Crash after denied capability", &trail)
            .expect("report generation should succeed");
        let value = serde_json::to_value(&report).expect("report should serialize");

        assert!(value.get("root_cause").is_some());
        assert!(value.get("steps_to_reproduce").is_some());
        assert!(value.get("severity").is_some());
    }
}
