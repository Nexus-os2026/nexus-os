//! BUG-DROP verification — the per-goal `model_override` must survive end-to-end
//! from `AgentGoal` construction through `CognitiveRuntime::assign_goal` and
//! `get_agent_status`, so the downstream `resolve_agent_llm_route` (Tauri side)
//! can read it and force the dropdown's model selection.
//!
//! These tests guard the data plumbing. They do NOT assert consumption at an
//! LLM call site — that lives in `app/src-tauri/src/lib.rs::resolve_agent_llm_route`
//! via thread-local `ACTIVE_AGENT_LLM_ROUTE` and is covered by the Tauri crate.

use nexus_kernel::cognitive::loop_runtime::CollectingEmitter;
use nexus_kernel::cognitive::{AgentGoal, CognitiveRuntime, LoopConfig};
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::supervisor::Supervisor;
use std::sync::{Arc, Mutex};

fn make_supervisor_with_agent() -> (Arc<Mutex<Supervisor>>, String) {
    let mut sup = Supervisor::new();
    let manifest = AgentManifest {
        name: "bug-drop-agent".into(),
        version: "1.0.0".into(),
        capabilities: vec!["llm.query".into()],
        fuel_budget: 10000,
        autonomy_level: Some(3),
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        default_goal: None,
        llm_model: None,
        fuel_period_id: None,
        monthly_fuel_cap: None,
        allowed_endpoints: None,
        domain_tags: vec![],
        filesystem_permissions: vec![],
    };
    let id = sup.start_agent(manifest).unwrap();
    (Arc::new(Mutex::new(sup)), id.to_string())
}

fn make_runtime(sup: Arc<Mutex<Supervisor>>) -> CognitiveRuntime {
    let emitter = Arc::new(CollectingEmitter::new());
    let config = LoopConfig {
        max_cycles_per_goal: 10,
        max_consecutive_failures: 2,
        cycle_delay_ms: 0,
        fuel_reserve_threshold: 0.05,
        reflection_interval: 3,
    };
    CognitiveRuntime::new(sup, config, emitter)
}

#[test]
fn test_bug_drop_agentgoal_default_none() {
    let goal = AgentGoal::new("hello".into(), 5);
    assert!(
        goal.model_override.is_none(),
        "AgentGoal::new must default model_override to None; got {:?}",
        goal.model_override
    );
}

#[test]
fn test_bug_drop_agentgoal_clone_and_serde_preserve_override() {
    let mut goal = AgentGoal::new("inspect logs".into(), 5);
    goal.model_override = Some("gemma4:e4b".to_string());

    let cloned = goal.clone();
    assert_eq!(cloned.model_override.as_deref(), Some("gemma4:e4b"));

    let json = serde_json::to_string(&goal).expect("serialize AgentGoal");
    let restored: AgentGoal = serde_json::from_str(&json).expect("deserialize AgentGoal");
    assert_eq!(
        restored.model_override.as_deref(),
        Some("gemma4:e4b"),
        "serde round-trip must preserve model_override"
    );
}

#[test]
fn test_bug_drop_assign_goal_propagates_override_to_active_goal() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let runtime = make_runtime(sup);

    let mut goal = AgentGoal::new("run the task".into(), 5);
    goal.model_override = Some("gemma4:e4b".to_string());

    runtime
        .assign_goal(&agent_id, goal)
        .expect("assign_goal should accept model_override");

    let status = runtime
        .get_agent_status(&agent_id)
        .expect("agent status must be available after assign_goal");
    let active = status
        .active_goal
        .expect("active_goal must be populated after assign_goal");

    assert_eq!(
        active.model_override.as_deref(),
        Some("gemma4:e4b"),
        "BUG-DROP regression: model_override dropped between assign_goal and get_agent_status"
    );
}

#[test]
fn test_bug_drop_assign_goal_without_override_stays_none() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let runtime = make_runtime(sup);

    let goal = AgentGoal::new("default-path goal".into(), 5);
    assert!(goal.model_override.is_none());

    runtime.assign_goal(&agent_id, goal).expect("assign_goal");

    let status = runtime.get_agent_status(&agent_id).unwrap();
    let active = status.active_goal.expect("active_goal populated");

    assert!(
        active.model_override.is_none(),
        "BUG-DROP: goals without an override must leave model_override=None so \
         the manifest/auto-resolver fallback path stays in effect"
    );
}
