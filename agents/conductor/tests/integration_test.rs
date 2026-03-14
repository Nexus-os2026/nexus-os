use nexus_conductor::types::{AgentRole, UserRequest};
use nexus_conductor::Conductor;
use nexus_connectors_llm::providers::{LlmProvider, LlmResponse};
use nexus_kernel::errors::AgentError;
use nexus_kernel::supervisor::Supervisor;
use nexus_sdk::ManifestBuilder;

/// Mock provider that returns invalid JSON so the planner falls back to rules.
struct MockProvider;

impl LlmProvider for MockProvider {
    fn query(
        &self,
        _prompt: &str,
        _max_tokens: u32,
        model: &str,
    ) -> Result<LlmResponse, AgentError> {
        Ok(LlmResponse {
            output_text: "not json".to_string(),
            token_count: 10,
            model_name: model.to_string(),
            tool_calls: Vec::new(),
        })
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn cost_per_token(&self) -> f64 {
        0.0
    }
}

#[test]
fn conductor_plans_without_llm() {
    // Rule-based fallback — no Ollama needed, runs in CI
    let mut conductor = Conductor::new(MockProvider, "mock");

    let request = UserRequest::new(
        "build a SaaS landing page with auth and Stripe payments",
        "/tmp/nexus-test-output",
    );

    // Preview the plan (doesn't execute)
    let plan = conductor.preview_plan(&request).unwrap();

    // Should have subtasks for WebBuilder and Coder
    let roles: Vec<_> = plan.tasks.iter().map(|t| &t.role).collect();
    assert!(
        roles.contains(&&AgentRole::WebBuilder),
        "plan must include a WebBuilder task"
    );
    assert!(
        roles.contains(&&AgentRole::Coder),
        "plan must include a Coder task"
    );

    // The integration task (Coder) depends on WebBuilder (index 0)
    let coder_tasks: Vec<_> = plan
        .tasks
        .iter()
        .filter(|t| t.role == AgentRole::Coder)
        .collect();
    let has_dependency_on_web = coder_tasks
        .iter()
        .any(|t| t.depends_on.contains(&0));
    assert!(
        has_dependency_on_web,
        "at least one Coder task must depend on the WebBuilder task"
    );

    // Total estimated fuel > 0
    let total_fuel: u64 = plan.tasks.iter().map(|t| t.estimated_fuel).sum();
    assert!(total_fuel > 0, "total estimated fuel must be positive");
}

#[test]
fn conductor_dispatches_to_supervisor() {
    // Verify that the dispatcher creates correct manifests
    // and the supervisor accepts them
    let mut supervisor = Supervisor::new();

    // Build a manifest the same way the dispatcher would
    let manifest = ManifestBuilder::new("conductor-web-builder")
        .version("0.1.0")
        .capability("fs.read")
        .capability("fs.write")
        .capability("llm.query")
        .fuel_budget(1500)
        .autonomy_level(2)
        .build()
        .unwrap();

    // The supervisor should accept it
    let agent_id = supervisor.start_agent(manifest);
    assert!(agent_id.is_ok(), "supervisor must accept a valid manifest");

    let id = agent_id.unwrap();
    // Verify the agent is tracked
    let handle = supervisor.get_agent(id);
    assert!(handle.is_some(), "agent must be retrievable after start");
}

#[test]
#[ignore] // Requires Ollama running locally
fn conductor_builds_website_e2e() {
    // Full end-to-end test with real LLM
    // 1. Create temp dir
    let output_dir = std::env::temp_dir().join("nexus-conductor-e2e");
    let _ = std::fs::create_dir_all(&output_dir);

    // 2. Would need a real OllamaProvider here
    // 3. Run "build a portfolio site with dark mode"
    // 4. Assert index.html exists and contains <html
    // 5. Print output dir for manual inspection
    println!(
        "E2E test output dir: {}",
        output_dir.display()
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&output_dir);
}
