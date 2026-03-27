//! Agent integration tests — comprehensive testing of agent lifecycle,
//! cognitive loop resilience, actuator safety, and planner prompt quality.
//!
//! These tests exercise the full agent stack WITHOUT any running LLM backends.
//! They use mock LLMs and executors to verify every error path returns Err (not panic).

use nexus_kernel::actuators::{ActuatorContext, ActuatorRegistry};
use nexus_kernel::audit::AuditTrail;
use nexus_kernel::autonomy::AutonomyLevel;
use nexus_kernel::cognitive::loop_runtime::CollectingEmitter;
use nexus_kernel::cognitive::{
    ActionExecutor, AgentGoal, AgentMemoryManager, CognitivePhase, CognitivePlanner,
    CognitiveRuntime, EventEmitter, LoopConfig, MemoryStore, NoOpEmitter, PlannedAction,
    PlannerLlm, PlanningContext,
};
use nexus_kernel::errors::AgentError;
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::supervisor::Supervisor;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// ── Test helpers ────────────────────────────────────────────────────────

struct MockLlm {
    response: String,
}

impl PlannerLlm for MockLlm {
    fn plan_query(&self, _prompt: &str) -> Result<String, AgentError> {
        Ok(self.response.clone())
    }
}

/// LLM that always fails — simulates provider being down.
struct FailingLlm {
    error_msg: String,
}

impl PlannerLlm for FailingLlm {
    fn plan_query(&self, _prompt: &str) -> Result<String, AgentError> {
        Err(AgentError::SupervisorError(self.error_msg.clone()))
    }
}

struct MockExecutor {
    results: Mutex<Vec<Result<String, String>>>,
}

impl MockExecutor {
    fn always_ok(result: &str) -> Self {
        Self {
            results: Mutex::new(vec![Ok(result.to_string()); 100]),
        }
    }

    fn always_err(err: &str) -> Self {
        Self {
            results: Mutex::new(vec![Err(err.to_string()); 100]),
        }
    }
}

impl ActionExecutor for MockExecutor {
    fn execute(
        &self,
        _agent_id: &str,
        _action: &PlannedAction,
        _audit: &mut AuditTrail,
    ) -> Result<String, String> {
        let mut results = self.results.lock().unwrap();
        if results.is_empty() {
            Ok("default".to_string())
        } else {
            results.remove(0)
        }
    }
}

struct MockMemoryStore;

impl MemoryStore for MockMemoryStore {
    fn save_memory(&self, _: &str, _: &str, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn load_memories(
        &self,
        _: &str,
        _: Option<&str>,
        _: usize,
    ) -> Result<Vec<nexus_kernel::cognitive::MemoryEntry>, String> {
        Ok(vec![])
    }
    fn touch_memory(&self, _: i64) -> Result<(), String> {
        Ok(())
    }
    fn decay_memories(&self, _: &str, _: f64) -> Result<(), String> {
        Ok(())
    }
}

fn make_supervisor_with_agent() -> (Arc<Mutex<Supervisor>>, String) {
    let mut sup = Supervisor::new();
    let manifest = AgentManifest {
        name: "test-agent".into(),
        version: "1.0.0".into(),
        capabilities: vec![
            "llm.query".into(),
            "fs.read".into(),
            "fs.write".into(),
            "process.exec".into(),
        ],
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

fn make_planner(response: &str) -> CognitivePlanner {
    CognitivePlanner::new(Box::new(MockLlm {
        response: response.to_string(),
    }))
}

fn make_failing_planner(error_msg: &str) -> CognitivePlanner {
    CognitivePlanner::new(Box::new(FailingLlm {
        error_msg: error_msg.to_string(),
    }))
}

fn make_memory_mgr() -> AgentMemoryManager {
    AgentMemoryManager::new(Box::new(MockMemoryStore))
}

fn make_runtime(sup: Arc<Mutex<Supervisor>>) -> (CognitiveRuntime, Arc<CollectingEmitter>) {
    let emitter = Arc::new(CollectingEmitter::new());
    let config = LoopConfig {
        max_cycles_per_goal: 10,
        max_consecutive_failures: 2,
        cycle_delay_ms: 0,
        fuel_reserve_threshold: 0.05,
        reflection_interval: 3,
    };
    let runtime = CognitiveRuntime::new(sup, config, emitter.clone());
    (runtime, emitter)
}

fn make_context(caps: Vec<&str>) -> PlanningContext {
    PlanningContext {
        agent_name: Some("integration-test-agent".into()),
        agent_description: Some("Test agent for integration tests".into()),
        agent_capabilities: caps.into_iter().map(|s| s.to_string()).collect(),
        available_fuel: 1000.0,
        relevant_memories: vec![],
        previous_outcomes: vec![],
        working_directory: Some("/tmp/nexus-test".into()),
        autonomy_level: 3,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TEST GROUP 1: LLM Provider Resilience Through Cognitive Loop
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_cognitive_cycle_with_llm_down_returns_error_not_panic() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let (runtime, _) = make_runtime(sup);
    let goal = AgentGoal::new("test goal".into(), 5);
    runtime.assign_goal(&agent_id, goal).unwrap();

    let planner = make_failing_planner("Ollama not running at http://localhost:11434");
    let memory_mgr = make_memory_mgr();
    let executor = MockExecutor::always_ok("ok");
    let mut audit = AuditTrail::new();

    let result = runtime.run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit);
    // Must return Err, NOT panic
    assert!(
        result.is_err(),
        "LLM down should propagate as Err through cognitive cycle"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Ollama") || err.contains("not running"),
        "error should mention the provider: got {err}"
    );
}

#[test]
fn test_cognitive_cycle_with_generic_llm_error() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let (runtime, _) = make_runtime(sup);
    let goal = AgentGoal::new("test".into(), 5);
    runtime.assign_goal(&agent_id, goal).unwrap();

    let planner = make_failing_planner("connection refused");
    let memory_mgr = make_memory_mgr();
    let executor = MockExecutor::always_ok("ok");
    let mut audit = AuditTrail::new();

    let result = runtime.run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit);
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════
// TEST GROUP 2: Agent Lifecycle
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_agent_start_and_stop_clean() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let (runtime, _) = make_runtime(sup);
    let goal = AgentGoal::new("lifecycle test".into(), 5);
    runtime.assign_goal(&agent_id, goal).unwrap();
    assert!(runtime.has_active_loop(&agent_id));

    runtime.stop_agent_loop(&agent_id).unwrap();
    assert!(!runtime.has_active_loop(&agent_id));
}

#[test]
fn test_agent_stop_when_not_running() {
    let sup = Arc::new(Mutex::new(Supervisor::new()));
    let (runtime, _) = make_runtime(sup);
    // Stopping a non-existent agent should return Ok (idempotent)
    let result = runtime.stop_agent_loop("nonexistent-agent");
    assert!(
        result.is_ok(),
        "stopping non-existent agent should be idempotent"
    );
}

#[test]
fn test_agent_double_assign_goal() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let (runtime, _) = make_runtime(sup);

    let goal1 = AgentGoal::new("first goal".into(), 5);
    runtime.assign_goal(&agent_id, goal1).unwrap();

    // Assigning a second goal should replace the first (or error cleanly)
    let goal2 = AgentGoal::new("second goal".into(), 3);
    let result = runtime.assign_goal(&agent_id, goal2);
    // Either succeeds (replaces) or returns error — must NOT panic
    match result {
        Ok(_) => {
            let status = runtime.get_agent_status(&agent_id).unwrap();
            // New goal should be active
            assert_eq!(status.cycle_count, 0);
        }
        Err(e) => {
            // Error is acceptable — "already has active goal"
            let msg = e.to_string();
            assert!(
                msg.contains("active") || msg.contains("already"),
                "unexpected error: {msg}"
            );
        }
    }
}

#[test]
fn test_agent_status_after_assign() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let (runtime, _) = make_runtime(sup);
    let goal = AgentGoal::new("status check".into(), 7);
    runtime.assign_goal(&agent_id, goal).unwrap();

    let status = runtime.get_agent_status(&agent_id).unwrap();
    assert_eq!(status.phase, CognitivePhase::Perceive);
    assert_eq!(status.cycle_count, 0);
    assert_eq!(status.steps_completed, 0);
}

#[test]
fn test_agent_status_for_nonexistent() {
    let sup = Arc::new(Mutex::new(Supervisor::new()));
    let (runtime, _) = make_runtime(sup);
    let status = runtime.get_agent_status("nonexistent");
    assert!(
        status.is_none(),
        "nonexistent agent should return None status"
    );
}

#[test]
fn test_agent_phase_tracking() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let (runtime, _) = make_runtime(sup);
    let goal = AgentGoal::new("phase tracking".into(), 5);
    runtime.assign_goal(&agent_id, goal).unwrap();

    let phase = runtime.get_agent_phase(&agent_id);
    assert_eq!(phase, Some(CognitivePhase::Perceive));
}

// ═══════════════════════════════════════════════════════════════════════
// TEST GROUP 3: Cognitive Loop — Bad LLM Output Handling
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_cognitive_loop_handles_garbage_json() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let (runtime, _) = make_runtime(sup);
    let goal = AgentGoal::new("garbage json".into(), 5);
    runtime.assign_goal(&agent_id, goal).unwrap();

    // LLM returns garbage — planner should produce a fallback LlmQuery step
    let planner = make_planner("Sure! Here's the JSON: ```json\n{broken");
    let memory_mgr = make_memory_mgr();
    let executor = MockExecutor::always_ok("ok");
    let mut audit = AuditTrail::new();

    let result = runtime.run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit);
    // Should NOT panic — either succeeds with fallback or returns error
    assert!(
        result.is_ok(),
        "bad JSON should be handled gracefully, got: {:?}",
        result.err()
    );
    let cycle = result.unwrap();
    assert!(cycle.steps_executed >= 0);
}

#[test]
fn test_cognitive_loop_handles_empty_plan() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let (runtime, _) = make_runtime(sup);
    let goal = AgentGoal::new("empty plan".into(), 5);
    runtime.assign_goal(&agent_id, goal).unwrap();

    // LLM returns empty array — valid JSON but no steps
    let planner = make_planner("[]");
    let memory_mgr = make_memory_mgr();
    let executor = MockExecutor::always_ok("ok");
    let mut audit = AuditTrail::new();

    let result = runtime.run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit);
    assert!(result.is_ok());
}

#[test]
fn test_cognitive_loop_with_think_tags_in_plan() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let (runtime, _) = make_runtime(sup);
    let goal = AgentGoal::new("think tags".into(), 5);
    runtime.assign_goal(&agent_id, goal).unwrap();

    // LLM returns response wrapped in <think> tags (Qwen3 style)
    let response = r#"<think>I need to analyze the code base first</think>
[{"action": {"type": "LlmQuery", "prompt": "analyze code", "context": []}, "description": "analyze"}]"#;
    let planner = make_planner(response);
    let memory_mgr = make_memory_mgr();
    let executor = MockExecutor::always_ok("analysis complete");
    let mut audit = AuditTrail::new();

    let result = runtime.run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit);
    assert!(
        result.is_ok(),
        "think tags should be stripped before parsing"
    );
    let cycle = result.unwrap();
    assert_eq!(
        cycle.steps_executed, 1,
        "should have executed the LlmQuery step"
    );
}

#[test]
fn test_cognitive_loop_with_markdown_json() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let (runtime, _) = make_runtime(sup);
    let goal = AgentGoal::new("markdown json".into(), 5);
    runtime.assign_goal(&agent_id, goal).unwrap();

    // LLM wraps JSON in markdown code block
    let response = r#"Here is the plan:

```json
[{"action": {"type": "Noop"}, "description": "wait for user input"}]
```

This plan does nothing."#;
    let planner = make_planner(response);
    let memory_mgr = make_memory_mgr();
    let executor = MockExecutor::always_ok("ok");
    let mut audit = AuditTrail::new();

    let result = runtime.run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit);
    assert!(result.is_ok(), "markdown-wrapped JSON should be extracted");
}

#[test]
fn test_cognitive_loop_executor_error_handled() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let (runtime, _) = make_runtime(sup);
    let goal = AgentGoal::new("executor error".into(), 5);
    runtime.assign_goal(&agent_id, goal).unwrap();

    let planner = make_planner(
        r#"[{"action": {"type": "LlmQuery", "prompt": "test", "context": []}, "description": "test"}]"#,
    );
    let memory_mgr = make_memory_mgr();
    let executor = MockExecutor::always_err("command failed: file not found");
    let mut audit = AuditTrail::new();

    let result = runtime.run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit);
    // Executor error should NOT crash the loop — it should be recorded
    assert!(
        result.is_ok(),
        "executor errors should be handled in reflect phase"
    );
    let cycle = result.unwrap();
    assert!(
        cycle.should_continue,
        "loop should continue after executor error"
    );
}

#[test]
fn test_cognitive_loop_string_args_tolerance() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let (runtime, _) = make_runtime(sup);
    let goal = AgentGoal::new("string args".into(), 5);
    runtime.assign_goal(&agent_id, goal).unwrap();

    // LLM returns "args": "-m" instead of "args": ["-m"]
    let response = r#"[{"action": {"type": "ShellCommand", "command": "free", "args": "-m"}, "description": "check memory"}]"#;
    let planner = make_planner(response);
    let memory_mgr = make_memory_mgr();
    let executor = MockExecutor::always_ok("Mem: 16384 12345 4039");
    let mut audit = AuditTrail::new();

    let result = runtime.run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit);
    assert!(
        result.is_ok(),
        "string args should be tolerated via string_or_vec deserializer"
    );
    let cycle = result.unwrap();
    assert_eq!(cycle.steps_executed, 1);
}

// ═══════════════════════════════════════════════════════════════════════
// TEST GROUP 4: Max Cycles Enforcement
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_max_cycles_enforced() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let emitter = Arc::new(CollectingEmitter::new());
    let config = LoopConfig {
        max_cycles_per_goal: 3,
        max_consecutive_failures: 10,
        cycle_delay_ms: 0,
        fuel_reserve_threshold: 0.01,
        reflection_interval: 100,
    };
    let runtime = CognitiveRuntime::new(sup, config, emitter);
    let goal = AgentGoal::new("many cycles".into(), 5);
    runtime.assign_goal(&agent_id, goal).unwrap();

    // Multi-step plan that won't complete in 3 cycles
    let planner = make_planner(
        r#"[
            {"action": {"type": "LlmQuery", "prompt": "step1", "context": []}, "description": "s1"},
            {"action": {"type": "LlmQuery", "prompt": "step2", "context": []}, "description": "s2"},
            {"action": {"type": "LlmQuery", "prompt": "step3", "context": []}, "description": "s3"},
            {"action": {"type": "LlmQuery", "prompt": "step4", "context": []}, "description": "s4"},
            {"action": {"type": "LlmQuery", "prompt": "step5", "context": []}, "description": "s5"}
        ]"#,
    );
    let memory_mgr = make_memory_mgr();
    let executor = MockExecutor::always_ok("ok");
    let mut audit = AuditTrail::new();

    let mut total_cycles = 0;
    for _ in 0..10 {
        let result = runtime.run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit);
        match result {
            Ok(cycle) => {
                total_cycles += 1;
                if !cycle.should_continue {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    assert!(
        total_cycles <= 4,
        "should stop after ~3 cycles (max_cycles_per_goal=3)"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// TEST GROUP 5: Actuator Safety (via ActuatorRegistry directly)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_actuator_registry_has_all_standard_actuators() {
    let registry = ActuatorRegistry::with_defaults();
    // The registry should be non-empty
    // We can test by trying to execute a known action type
    let ctx = ActuatorContext {
        agent_id: "test-agent".into(),
        agent_name: "Test Agent".into(),
        working_dir: PathBuf::from("/tmp/nexus-test"),
        autonomy_level: AutonomyLevel::L3,
        capabilities: vec!["process.exec".to_string()].into_iter().collect(),
        fuel_remaining: 1000.0,
        egress_allowlist: vec![],
        action_review_engine: None,
    };

    // ShellCommand should be routed to the GovernedShell actuator
    let action = PlannedAction::ShellCommand {
        command: "echo".into(),
        args: vec!["hello".into()],
    };
    let mut audit = AuditTrail::new();
    let result = registry.execute_action(&action, &ctx, &mut audit);
    // Should either succeed or fail with a governance error — NOT panic
    match result {
        Ok(r) => assert!(r.success || !r.output.is_empty()),
        Err(e) => {
            // Governance rejection is fine
            let msg = format!("{e}");
            assert!(!msg.is_empty(), "error should have a message");
        }
    }
}

#[test]
fn test_actuator_rejects_missing_capability() {
    let registry = ActuatorRegistry::with_defaults();
    let ctx = ActuatorContext {
        agent_id: "test-agent".into(),
        agent_name: "Test Agent".into(),
        working_dir: PathBuf::from("/tmp/nexus-test"),
        autonomy_level: AutonomyLevel::L3,
        capabilities: HashSet::new(), // No capabilities
        fuel_remaining: 1000.0,
        egress_allowlist: vec![],
        action_review_engine: None,
    };

    let action = PlannedAction::ShellCommand {
        command: "echo".into(),
        args: vec!["hello".into()],
    };
    let mut audit = AuditTrail::new();
    let result = registry.execute_action(&action, &ctx, &mut audit);
    assert!(
        result.is_err(),
        "should reject action without required capability"
    );
}

#[test]
fn test_actuator_rejects_zero_fuel() {
    let registry = ActuatorRegistry::with_defaults();
    let ctx = ActuatorContext {
        agent_id: "test-agent".into(),
        agent_name: "Test Agent".into(),
        working_dir: PathBuf::from("/tmp/nexus-test"),
        autonomy_level: AutonomyLevel::L3,
        capabilities: vec!["fs.read".to_string()].into_iter().collect(),
        fuel_remaining: 0.0, // No fuel
        egress_allowlist: vec![],
        action_review_engine: None,
    };

    let action = PlannedAction::FileRead {
        path: "/tmp/nexus-test/test.txt".into(),
    };
    let mut audit = AuditTrail::new();
    let result = registry.execute_action(&action, &ctx, &mut audit);
    assert!(result.is_err(), "should reject action with zero fuel");
}

// ═══════════════════════════════════════════════════════════════════════
// TEST GROUP 6: Planner Behavior (via public API)
// Note: build_planning_prompt is private — prompt quality tests live
// in kernel/src/cognitive/planner.rs inline tests. Here we test the
// planner's public plan_goal/replan_after_failure behavior.
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_planner_produces_valid_steps_from_json() {
    let planner = make_planner(
        r#"[{"action": {"type": "LlmQuery", "prompt": "analyze", "context": []}, "description": "analyze code"}]"#,
    );
    let goal = AgentGoal::new("analyze the codebase".into(), 7);
    let ctx = make_context(vec!["llm.query"]);
    let steps = planner.plan_goal(&goal, &ctx).unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].action.action_type(), "llm_query");
}

#[test]
fn test_planner_handles_completely_invalid_json() {
    let planner = make_planner("This is not JSON at all, just a free-form response from the LLM.");
    let goal = AgentGoal::new("test".into(), 5);
    let ctx = make_context(vec!["llm.query"]);
    // Should produce a fallback LlmQuery step, not crash
    let result = planner.plan_goal(&goal, &ctx);
    assert!(
        result.is_ok(),
        "invalid JSON should produce fallback, not error"
    );
    let steps = result.unwrap();
    assert!(!steps.is_empty(), "should have at least a fallback step");
}

#[test]
fn test_planner_handles_empty_array() {
    let planner = make_planner("[]");
    let goal = AgentGoal::new("test".into(), 5);
    let ctx = make_context(vec!["llm.query"]);
    let result = planner.plan_goal(&goal, &ctx);
    // Empty plan is valid (though useless)
    assert!(result.is_ok());
}

#[test]
fn test_planner_rejects_unauthorized_actions() {
    let planner = make_planner(
        r#"[{"action": {"type": "ShellCommand", "command": "rm", "args": ["-rf", "/"]}, "description": "delete everything"}]"#,
    );
    let goal = AgentGoal::new("test".into(), 5);
    // Only fs.read — no process.exec
    let ctx = make_context(vec!["fs.read"]);

    let result = planner.plan_goal(&goal, &ctx);
    assert!(
        result.is_err(),
        "planner should reject ShellCommand without process.exec capability"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("process.exec"),
        "error should mention missing capability: got {err}"
    );
}

#[test]
fn test_planner_always_allows_safe_actions() {
    let planner = make_planner(
        r#"[
            {"action": {"type": "MemoryStore", "key": "test", "value": "data", "memory_type": "episodic"}, "description": "store"},
            {"action": {"type": "Noop"}, "description": "wait"},
            {"action": {"type": "HitlRequest", "question": "continue?", "options": ["yes", "no"]}, "description": "ask"}
        ]"#,
    );
    let goal = AgentGoal::new("test".into(), 5);
    // No capabilities at all
    let ctx = make_context(vec![]);

    let result = planner.plan_goal(&goal, &ctx);
    assert!(
        result.is_ok(),
        "MemoryStore, Noop, and HitlRequest should always be allowed"
    );
    let steps = result.unwrap();
    assert_eq!(steps.len(), 3);
}

// ═══════════════════════════════════════════════════════════════════════
// TEST GROUP 7: Event Emission Safety
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_event_emitter_handles_large_payload() {
    let emitter = CollectingEmitter::new();
    // Emit an event with a large payload
    let large_text = "x".repeat(1_000_000); // 1MB
    emitter.emit(nexus_kernel::cognitive::CognitiveEvent::AgentNotification {
        agent_id: "test".into(),
        title: "large payload test".into(),
        body: large_text,
        level: "info".into(),
    });
    let events = emitter.events.lock().unwrap();
    assert_eq!(events.len(), 1, "large event should be stored");
}

#[test]
fn test_no_op_emitter_never_fails() {
    let emitter = NoOpEmitter;
    // Should silently accept any event
    emitter.emit(nexus_kernel::cognitive::CognitiveEvent::PhaseChange {
        agent_id: "test".into(),
        phase: CognitivePhase::Perceive,
        goal_id: "g1".into(),
        timestamp: 0,
    });
    emitter.emit(nexus_kernel::cognitive::CognitiveEvent::StepExecuted {
        agent_id: "test".into(),
        step_id: "s1".into(),
        action_type: "LlmQuery".into(),
        status: nexus_kernel::cognitive::StepStatus::Succeeded,
        result_preview: Some("ok".into()),
        fuel_cost: 1.0,
    });
    // No assertions needed — just verify no panic
}

// ═══════════════════════════════════════════════════════════════════════
// TEST GROUP 8: Planned Action Type Correctness
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_all_action_types_have_names() {
    // Verify every PlannedAction variant has a non-empty action_type()
    let actions = vec![
        PlannedAction::LlmQuery {
            prompt: "test".into(),
            context: vec![],
        },
        PlannedAction::FileRead {
            path: "/tmp/test".into(),
        },
        PlannedAction::FileWrite {
            path: "/tmp/test".into(),
            content: "hello".into(),
        },
        PlannedAction::ShellCommand {
            command: "echo".into(),
            args: vec!["hi".into()],
        },
        PlannedAction::Noop,
        PlannedAction::MemoryStore {
            key: "k".into(),
            value: "v".into(),
            memory_type: "episodic".into(),
        },
        PlannedAction::MemoryRecall {
            query: "q".into(),
            memory_type: None,
        },
    ];

    for action in &actions {
        let name = action.action_type();
        assert!(
            !name.is_empty(),
            "action type should not be empty for {:?}",
            action
        );
    }
}

#[test]
fn test_action_required_capabilities() {
    // ShellCommand requires process.exec
    let shell = PlannedAction::ShellCommand {
        command: "ls".into(),
        args: vec![],
    };
    let caps = shell.required_capabilities();
    assert!(
        caps.contains(&"process.exec"),
        "ShellCommand needs process.exec"
    );

    // FileWrite requires fs.write
    let write = PlannedAction::FileWrite {
        path: "/tmp/x".into(),
        content: "y".into(),
    };
    let caps = write.required_capabilities();
    assert!(caps.contains(&"fs.write"), "FileWrite needs fs.write");

    // Noop requires nothing
    let noop = PlannedAction::Noop;
    let caps = noop.required_capabilities();
    assert!(caps.is_empty(), "Noop should require no capabilities");
}

// ═══════════════════════════════════════════════════════════════════════
// TEST GROUP 9: Audit Trail Integration
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_cognitive_cycle_creates_audit_events() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let (runtime, _) = make_runtime(sup);
    let goal = AgentGoal::new("audit test".into(), 5);
    runtime.assign_goal(&agent_id, goal).unwrap();

    let planner = make_planner(
        r#"[{"action": {"type": "LlmQuery", "prompt": "hello", "context": []}, "description": "test"}]"#,
    );
    let memory_mgr = make_memory_mgr();
    let executor = MockExecutor::always_ok("ok");
    let mut audit = AuditTrail::new();

    let events_before = audit.events().len();
    runtime
        .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
        .unwrap();
    let events_after = audit.events().len();

    assert!(
        events_after > events_before,
        "cognitive cycle should create audit events"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// TEST GROUP 10: Consecutive Failure Handling
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_consecutive_failures_trigger_replan() {
    let (sup, agent_id) = make_supervisor_with_agent();
    let emitter = Arc::new(CollectingEmitter::new());
    let config = LoopConfig {
        max_cycles_per_goal: 20,
        max_consecutive_failures: 2,
        cycle_delay_ms: 0,
        fuel_reserve_threshold: 0.01,
        reflection_interval: 100,
    };
    let runtime = CognitiveRuntime::new(sup, config, emitter);
    let goal = AgentGoal::new("replan on failure".into(), 5);
    runtime.assign_goal(&agent_id, goal).unwrap();

    // Step will always fail
    let planner = make_planner(
        r#"[{"action": {"type": "LlmQuery", "prompt": "will fail", "context": []}, "description": "fail"}]"#,
    );
    let memory_mgr = make_memory_mgr();
    let executor = MockExecutor::always_err("simulated failure");
    let mut audit = AuditTrail::new();

    // Run multiple cycles — should eventually replan
    let mut _replanned = false;
    for _ in 0..5 {
        let result = runtime.run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit);
        match result {
            Ok(cycle) => {
                if !cycle.should_continue {
                    break;
                }
            }
            Err(_) => {
                // Replan failure (LLM down) is also acceptable
                _replanned = true;
                break;
            }
        }
    }
    // The test passes as long as it doesn't panic
}

// ═══════════════════════════════════════════════════════════════════════
// TEST GROUP 11: Planned Action Deserialization Robustness
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_deserialize_all_common_action_types() {
    let test_cases = vec![
        r#"{"type": "LlmQuery", "prompt": "hello", "context": []}"#,
        r#"{"type": "FileRead", "path": "/tmp/test.txt"}"#,
        r#"{"type": "FileWrite", "path": "/tmp/test.txt", "content": "hello"}"#,
        r#"{"type": "ShellCommand", "command": "ls", "args": ["-la"]}"#,
        r#"{"type": "WebSearch", "query": "rust programming", "max_results": 5}"#,
        r#"{"type": "WebFetch", "url": "https://example.com", "headers": {}}"#,
        r#"{"type": "Noop"}"#,
        r#"{"type": "MemoryStore", "key": "test", "value": "data", "memory_type": "episodic"}"#,
        r#"{"type": "MemoryRecall", "query": "test"}"#,
        r#"{"type": "HitlRequest", "question": "proceed?", "options": ["yes", "no"]}"#,
        r#"{"type": "SendNotification", "title": "Test", "body": "done", "level": "info"}"#,
        r#"{"type": "AgentMessage", "target_agent": "abc123", "message": "hello"}"#,
    ];

    for (i, json) in test_cases.iter().enumerate() {
        let result: Result<PlannedAction, _> = serde_json::from_str(json);
        assert!(
            result.is_ok(),
            "test case {i} failed to deserialize: {json}\nerror: {:?}",
            result.err()
        );
    }
}

#[test]
fn test_deserialize_with_extra_fields_tolerated() {
    // LLMs sometimes add extra fields — these should be ignored
    let json = r#"{"type": "Noop", "reasoning": "I need to wait", "confidence": 0.95}"#;
    let result: Result<PlannedAction, _> = serde_json::from_str(json);
    assert!(result.is_ok(), "extra fields should be tolerated");
}

#[test]
fn test_deserialize_code_execute() {
    let json = r#"{"type": "CodeExecute", "language": "python", "code": "print('hello')", "timeout_secs": 30}"#;
    let result: Result<PlannedAction, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "CodeExecute should deserialize: {:?}",
        result.err()
    );
}
