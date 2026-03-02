use coding_agent::{
    ApprovalGate, CodingAgent, CodingAgentConfig, CodingAgentManifest, CodingDependencies,
    CodingIoProxy, CodingPlanner, IterationPlan, PlanningContext, ProposedWrite, TestExecution,
};
use nexus_kernel::errors::AgentError;
use std::collections::VecDeque;

struct ScriptedPlanner {
    steps: VecDeque<IterationPlan>,
}

impl ScriptedPlanner {
    fn new(steps: Vec<IterationPlan>) -> Self {
        Self {
            steps: VecDeque::from(steps),
        }
    }
}

impl CodingPlanner for ScriptedPlanner {
    fn plan(&mut self, _context: PlanningContext) -> Result<IterationPlan, AgentError> {
        self.steps
            .pop_front()
            .ok_or_else(|| AgentError::SupervisorError("scripted plan exhausted".to_string()))
    }
}

struct ScriptedIo {
    outcomes: VecDeque<TestExecution>,
}

impl ScriptedIo {
    fn new(outcomes: Vec<TestExecution>) -> Self {
        Self {
            outcomes: VecDeque::from(outcomes),
        }
    }
}

impl CodingIoProxy for ScriptedIo {
    fn read_file(&mut self, _relative_path: &str) -> Result<String, AgentError> {
        Ok("pub fn value() -> u32 { 1 }".to_string())
    }

    fn write_file(&mut self, _relative_path: &str, _content: &str) -> Result<(), AgentError> {
        Ok(())
    }

    fn run_tests(&mut self, _command: &str) -> Result<TestExecution, AgentError> {
        self.outcomes
            .pop_front()
            .ok_or_else(|| AgentError::SupervisorError("missing scripted test result".to_string()))
    }
}

struct ApproveAll;

impl ApprovalGate for ApproveAll {
    fn approve_write(&mut self, _write: &ProposedWrite, _iteration: u32) -> bool {
        true
    }

    fn approve_test_run(&mut self, _command: &str, _iteration: u32) -> bool {
        true
    }
}

#[test]
fn test_coding_agent_dry_run_integration() {
    let manifest = CodingAgentManifest {
        name: "coding-agent".to_string(),
        version: "2.0.0".to_string(),
        capabilities: vec![
            "fs.read".to_string(),
            "fs.write".to_string(),
            "process.exec".to_string(),
        ],
        fuel_budget: 200,
        schedule: None,
        llm_model: Some("claude-sonnet-4-5".to_string()),
        config: CodingAgentConfig {
            repo_path: ".".to_string(),
            objective: "Fix failing tests".to_string(),
            test_command: "cargo test -p sample".to_string(),
            max_iterations: 3,
            target_files: vec!["src/lib.rs".to_string()],
        },
    };

    let planner = ScriptedPlanner::new(vec![
        IterationPlan {
            summary: "first fix".to_string(),
            read_paths: vec!["src/lib.rs".to_string()],
            writes: vec![ProposedWrite {
                path: "src/lib.rs".to_string(),
                content: "pub fn value() -> u32 { 2 }".to_string(),
                summary: "adjust implementation".to_string(),
            }],
            run_tests: true,
        },
        IterationPlan {
            summary: "second fix".to_string(),
            read_paths: vec!["src/lib.rs".to_string()],
            writes: vec![],
            run_tests: true,
        },
    ]);

    let io = ScriptedIo::new(vec![
        TestExecution {
            success: false,
            exit_code: Some(101),
            stdout: "running 5 tests".to_string(),
            stderr: "1 test failed".to_string(),
        },
        TestExecution {
            success: true,
            exit_code: Some(0),
            stdout: "all tests passed".to_string(),
            stderr: String::new(),
        },
    ]);

    let dependencies = CodingDependencies {
        planner: Box::new(planner),
        io: Box::new(io),
        approval: Box::new(ApproveAll),
    };
    let mut agent = CodingAgent::with_dependencies(manifest, true, dependencies);

    let report = agent.run().expect("coding-agent run should complete");
    assert!(report.success);
    assert_eq!(report.iterations, 2);
    assert_eq!(report.modified_files, vec!["src/lib.rs".to_string()]);
    assert!(report.audit_events.len() >= 6);
}
