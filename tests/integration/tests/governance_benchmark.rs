use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::{LlmProvider, LlmResponse};
use nexus_kernel::errors::AgentError;
use std::collections::HashSet;
use std::hint::black_box;
use std::time::Instant;

/// Minimal provider that does near-zero work so the measurement isolates
/// governance overhead (audit hashing, safety supervisor heartbeat, drift
/// detection, PII redaction, output firewall, KPI monitoring).
#[derive(Debug, Clone, Copy)]
struct MinimalProvider;

impl LlmProvider for MinimalProvider {
    fn query(
        &self,
        _prompt: &str,
        _max_tokens: u32,
        model: &str,
    ) -> Result<LlmResponse, AgentError> {
        Ok(LlmResponse {
            output_text: "ok".to_string(),
            token_count: 512,
            model_name: model.to_string(),
            tool_calls: Vec::new(),
        })
    }

    fn name(&self) -> &str {
        "minimal-provider"
    }

    fn cost_per_token(&self) -> f64 {
        0.0
    }
}

/// Measures the absolute cost of the governance stack per LLM call.
///
/// Budget: 50ms per call. Real LLM calls take 500ms–5000ms, so 50ms of
/// governance overhead is under 10% in practice. If this fails, something
/// regressed in the governance path — not a flaky timing issue.
#[test]
fn governance_overhead_regression() {
    let iterations = 100_u64;

    let mut gateway = GovernedLlmGateway::new(MinimalProvider);
    let mut context = AgentRuntimeContext {
        agent_id: uuid::Uuid::new_v4(),
        capabilities: ["llm.query".to_string()]
            .into_iter()
            .collect::<HashSet<_>>(),
        fuel_remaining: iterations * 512 + 1024,
    };

    let start = Instant::now();
    for _ in 0..iterations {
        let response = gateway
            .query(
                &mut context,
                "benchmark governance path",
                512,
                "bench-model",
            )
            .expect("governed query should succeed");
        black_box(response);
    }
    let elapsed = start.elapsed();

    let per_call_ms = elapsed.as_secs_f64() * 1000.0 / iterations as f64;
    eprintln!(
        "governance overhead: {per_call_ms:.2}ms per call ({iterations} iterations, total {elapsed:?})"
    );

    assert!(
        per_call_ms < 50.0,
        "governance overhead regression: {per_call_ms:.2}ms per call exceeds 50ms budget"
    );
}
