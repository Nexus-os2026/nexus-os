use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::{LlmProvider, LlmResponse};
use nexus_kernel::errors::AgentError;
use std::collections::HashSet;
use std::hint::black_box;
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
struct BusyProvider {
    work: u64,
}

impl BusyProvider {
    fn execute_work(&self) -> u64 {
        let mut acc = 0_u64;
        for i in 0..self.work {
            acc = acc.wrapping_add(i ^ acc.rotate_left(5));
        }
        acc
    }
}

impl LlmProvider for BusyProvider {
    fn query(
        &self,
        _prompt: &str,
        max_tokens: u32,
        model: &str,
    ) -> Result<LlmResponse, AgentError> {
        let value = self.execute_work();
        black_box(value);
        Ok(LlmResponse {
            output_text: "ok".to_string(),
            token_count: max_tokens.min(8),
            model_name: model.to_string(),
            tool_calls: Vec::new(),
        })
    }

    fn name(&self) -> &str {
        "busy-provider"
    }

    fn cost_per_token(&self) -> f64 {
        0.0
    }
}

#[test]
fn test_governance_overhead_under_five_percent() {
    let iterations = 120_u64;
    let provider = BusyProvider { work: 150_000 };
    let prompt = "benchmark governance path";
    let model = "bench-model";

    let start_baseline = Instant::now();
    for _ in 0..iterations {
        let response = provider
            .query(prompt, 16, model)
            .expect("baseline provider query should succeed");
        black_box(response);
    }
    let baseline = start_baseline.elapsed();

    let mut gateway = GovernedLlmGateway::new(provider);
    let mut context = AgentRuntimeContext {
        agent_id: uuid::Uuid::new_v4(),
        capabilities: ["llm.query".to_string()]
            .into_iter()
            .collect::<HashSet<_>>(),
        fuel_remaining: iterations * 16 + 128,
    };

    let start_governed = Instant::now();
    for _ in 0..iterations {
        let response = gateway
            .query(&mut context, prompt, 16, model)
            .expect("governed query should succeed");
        black_box(response);
    }
    let governed = start_governed.elapsed();

    let ratio = governed.as_secs_f64() / baseline.as_secs_f64().max(0.000_001);
    assert!(
        ratio <= 1.05,
        "governance overhead exceeded 5% target: baseline={baseline:?}, governed={governed:?}, ratio={ratio:.4}"
    );
}
