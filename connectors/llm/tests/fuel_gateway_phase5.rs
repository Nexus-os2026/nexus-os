use nexus_connectors_llm::gateway::{
    AgentFuelBudgetConfig, AgentRuntimeContext, GovernedLlmGateway,
};
use nexus_connectors_llm::providers::MockProvider;
use nexus_kernel::errors::AgentError;
use nexus_kernel::fuel_hardening::{BudgetPeriodId, FuelViolation, ModelCost};
use std::collections::HashSet;

#[test]
fn phase5_gateway_enforces_cap_and_records_spend() {
    let provider = MockProvider::new();
    let mut gateway = GovernedLlmGateway::new(provider);
    let agent_id = uuid::Uuid::new_v4();

    gateway.configure_agent_budget(
        agent_id,
        AgentFuelBudgetConfig {
            period_id: BudgetPeriodId::new("2026-03"),
            cap_units: 30,
        },
    );
    gateway.set_model_cost(
        "mock-1",
        ModelCost {
            cost_per_1k_input: 1_000,
            cost_per_1k_output: 1_000,
        },
    );

    let mut capabilities = HashSet::new();
    capabilities.insert("llm.query".to_string());

    let mut context = AgentRuntimeContext {
        agent_id,
        capabilities,
        fuel_remaining: 1_000,
    };

    let first = gateway.query(&mut context, "small", 10, "mock-1");
    assert!(first.is_ok());

    let second = gateway.query(&mut context, "this request is larger", 64, "mock-1");
    assert!(matches!(
        second,
        Err(AgentError::FuelViolation {
            violation: FuelViolation::OverMonthlyCap,
            ..
        })
    ));

    let report = gateway
        .fuel_audit_report(agent_id)
        .expect("fuel report must be present");
    assert!(report.spent_units > report.cap_units);
}
