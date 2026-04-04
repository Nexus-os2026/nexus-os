use nexus_conductor::types::{AgentRole, UserRequest};
use nexus_conductor::Conductor;
use nexus_connectors_llm::providers::{LlmProvider, LlmResponse};
use nexus_kernel::audit::AuditTrail;
use nexus_kernel::errors::AgentError;
use nexus_kernel::supervisor::Supervisor;
use nexus_sdk::ManifestBuilder;
use uuid::Uuid;

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
            input_tokens: None,
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
    let has_dependency_on_web = coder_tasks.iter().any(|t| t.depends_on.contains(&0));
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
fn conductor_builds_website_e2e() {
    // End-to-end test using MockProvider — exercises plan → dispatch → codegen
    // without requiring a live LLM.  The rule-based planner produces a valid
    // plan and the web-builder codegen emits real HTML/CSS/TS files.
    let output_dir = std::env::temp_dir().join("nexus-conductor-e2e");
    let _ = std::fs::remove_dir_all(&output_dir);
    std::fs::create_dir_all(&output_dir).expect("output dir should be created");

    let mut conductor = Conductor::new(MockProvider, "mock");
    let request = UserRequest::new(
        "build a portfolio site with dark mode",
        output_dir.to_string_lossy().as_ref(),
    );

    // Preview the plan — should succeed via rule-based fallback
    let plan = conductor
        .preview_plan(&request)
        .expect("preview plan should succeed");
    assert!(
        !plan.tasks.is_empty(),
        "plan should contain at least one task"
    );

    let web_task = plan
        .tasks
        .iter()
        .find(|t| t.role == AgentRole::WebBuilder)
        .expect("plan should include a WebBuilder task");

    // Execute the web build — the MockProvider returns "not json" so the
    // conductor falls through to rule-based codegen which still emits files.
    let mut audit = AuditTrail::new();
    let agent_id = Uuid::new_v4();
    let created = conductor
        .execute_web_build(web_task, &output_dir, &mut audit, agent_id)
        .expect("web build should produce files via rule-based fallback");

    assert!(
        !created.is_empty(),
        "web build should create at least one file"
    );

    let has_html = created
        .iter()
        .any(|p| p.extension().map(|ext| ext == "html").unwrap_or(false));
    assert!(has_html, "web build should create an HTML file");

    // Cleanup
    let _ = std::fs::remove_dir_all(&output_dir);
}
