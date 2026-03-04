use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::{LlmProvider, LlmResponse, MockProvider};
use nexus_kernel::errors::AgentError;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

fn capabilities(values: &[&str]) -> HashSet<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[derive(Debug, Clone)]
struct CaptureProvider {
    captures: Arc<Mutex<Vec<String>>>,
}

impl CaptureProvider {
    fn new(captures: Arc<Mutex<Vec<String>>>) -> Self {
        Self { captures }
    }
}

impl LlmProvider for CaptureProvider {
    fn query(
        &self,
        prompt: &str,
        _max_tokens: u32,
        model: &str,
    ) -> Result<LlmResponse, AgentError> {
        if let Ok(mut captures) = self.captures.lock() {
            captures.push(prompt.to_string());
        }
        Ok(LlmResponse {
            output_text: "captured".to_string(),
            token_count: 1,
            model_name: model.to_string(),
            tool_calls: Vec::new(),
        })
    }

    fn name(&self) -> &str {
        "capture"
    }

    fn cost_per_token(&self) -> f64 {
        0.0
    }
}

#[test]
fn test_audit_redaction_events_do_not_contain_raw_secret() {
    let provider = MockProvider::new();
    let mut gateway = GovernedLlmGateway::new(provider);
    let mut context = AgentRuntimeContext {
        agent_id: Uuid::new_v4(),
        capabilities: capabilities(&["llm.query"]),
        fuel_remaining: 1_000,
    };
    let prompt = "email admin@company.com key sk-1234567890ABCDEFGHIJKLMNOP";

    let result = gateway.query(&mut context, prompt, 32, "mock-1");
    assert!(result.is_ok());

    for event in gateway.audit_trail().events() {
        let event_kind = event
            .payload
            .get("event_kind")
            .and_then(|value| value.as_str());
        if matches!(event_kind, Some("redaction.scanned" | "redaction.applied")) {
            let serialized = event.payload.to_string();
            assert!(!serialized.contains("admin@company.com"));
            assert!(!serialized.contains("sk-1234567890ABCDEFGHIJKLMNOP"));
        }
    }
}

#[test]
fn test_gateway_sends_redacted_payload_only() {
    let captures = Arc::new(Mutex::new(Vec::new()));
    let provider = CaptureProvider::new(Arc::clone(&captures));
    let mut gateway = GovernedLlmGateway::new(provider);
    let mut context = AgentRuntimeContext {
        agent_id: Uuid::new_v4(),
        capabilities: capabilities(&["llm.query"]),
        fuel_remaining: 1_000,
    };
    let prompt = "Contact a@b.com with token sk-1234567890ABCDEFGHIJKLMNOP";

    let result = gateway.query(&mut context, prompt, 16, "capture-model");
    assert!(result.is_ok());

    let captured = captures
        .lock()
        .expect("capture lock should not be poisoned")
        .last()
        .cloned()
        .unwrap_or_default();
    assert!(!captured.contains("a@b.com"));
    assert!(!captured.contains("sk-1234567890ABCDEFGHIJKLMNOP"));
    assert!(captured.contains("<redacted:email>"));
    assert!(captured.contains("<redacted:api_key>"));
    assert!(gateway.redaction_zero_pii_leakage_kpi());
}
