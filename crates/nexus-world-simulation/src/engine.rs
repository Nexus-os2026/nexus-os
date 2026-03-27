use serde::{Deserialize, Serialize};

use crate::outcome::{
    Recommendation, RiskAssessment, RiskLevel, SideEffect, SimulationResult, StepResult, StepRisk,
};
use crate::sandbox::SimulationSandbox;
use crate::scenario::{Scenario, ScenarioBranch, ScenarioStatus, SimulatedAction};

/// Configuration for the simulation engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    pub max_steps: u32,
    pub max_duration_secs: u64,
    pub max_concurrent: usize,
    pub allow_network: bool,
    pub sandbox_root: String,
    pub max_file_size_bytes: u64,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            max_steps: 50,
            max_duration_secs: 30,
            max_concurrent: 5,
            allow_network: false,
            sandbox_root: "/tmp/nexus-sim".into(),
            max_file_size_bytes: 10 * 1024 * 1024,
        }
    }
}

/// The World Simulation Engine — runs hypothetical scenarios in isolation.
pub struct SimulationEngine {
    simulations: Vec<Scenario>,
    history: Vec<Scenario>,
    branches: Vec<ScenarioBranch>,
    config: SimulationConfig,
}

impl SimulationEngine {
    pub fn new(config: SimulationConfig) -> Self {
        Self {
            simulations: Vec::new(),
            history: Vec::new(),
            branches: Vec::new(),
            config,
        }
    }

    /// Submit a scenario for simulation.
    pub fn submit(&mut self, scenario: Scenario) -> Result<String, SimulationError> {
        if scenario.actions.len() as u32 > self.config.max_steps {
            return Err(SimulationError::TooManySteps {
                requested: scenario.actions.len() as u32,
                maximum: self.config.max_steps,
            });
        }

        let active = self
            .simulations
            .iter()
            .filter(|s| matches!(s.status, ScenarioStatus::Running))
            .count();
        if active >= self.config.max_concurrent {
            return Err(SimulationError::ConcurrencyLimit {
                active,
                maximum: self.config.max_concurrent,
            });
        }

        let id = scenario.id.clone();
        self.simulations.push(scenario);
        Ok(id)
    }

    /// Run a submitted scenario.
    pub fn run_simulation(
        &mut self,
        scenario_id: &str,
    ) -> Result<SimulationResult, SimulationError> {
        let scenario = self
            .simulations
            .iter_mut()
            .find(|s| s.id == scenario_id)
            .ok_or_else(|| SimulationError::NotFound(scenario_id.into()))?;

        scenario.status = ScenarioStatus::Running;

        let start = std::time::Instant::now();
        let mut step_results = Vec::new();
        let mut sandbox = SimulationSandbox::new(&self.config);

        // Check preconditions
        for condition in &scenario.preconditions {
            if !sandbox.check_condition(condition) {
                let result = SimulationResult {
                    scenario_id: scenario_id.into(),
                    success: false,
                    step_results,
                    risk_assessment: RiskAssessment::high("Precondition not met"),
                    duration_ms: start.elapsed().as_millis() as u64,
                    recommendation: Recommendation::DoNotProceed {
                        reason: format!("Precondition failed: {}", condition.description),
                    },
                };
                scenario.status = ScenarioStatus::Completed {
                    result: result.clone(),
                };
                return Ok(result);
            }
        }

        // Execute each step
        let actions = scenario.actions.clone();
        for action in &actions {
            if start.elapsed().as_secs() > self.config.max_duration_secs {
                let result = SimulationResult {
                    scenario_id: scenario_id.into(),
                    success: false,
                    step_results,
                    risk_assessment: RiskAssessment::medium("Simulation timed out"),
                    duration_ms: start.elapsed().as_millis() as u64,
                    recommendation: Recommendation::NeedsReview {
                        reason: "Simulation exceeded time limit".into(),
                    },
                };
                if let Some(s) = self.simulations.iter_mut().find(|s| s.id == scenario_id) {
                    s.status = ScenarioStatus::Completed {
                        result: result.clone(),
                    };
                }
                return Ok(result);
            }

            // Check dependencies
            let deps_met = action.depends_on.iter().all(|dep| {
                step_results
                    .iter()
                    .any(|r: &StepResult| r.step == *dep && r.success)
            });

            if !deps_met {
                step_results.push(StepResult {
                    step: action.step,
                    success: false,
                    output: "Dependency not met".into(),
                    side_effects: Vec::new(),
                    risk: StepRisk::Low,
                });
                continue;
            }

            let step_result = sandbox.simulate_action(action);
            step_results.push(step_result);
        }

        let risk_assessment = assess_risk(&step_results);
        let recommendation = generate_recommendation(&step_results, &risk_assessment);
        let all_succeeded = step_results.iter().all(|r| r.success);

        let result = SimulationResult {
            scenario_id: scenario_id.into(),
            success: all_succeeded,
            step_results,
            risk_assessment,
            duration_ms: start.elapsed().as_millis() as u64,
            recommendation,
        };

        // Move to history
        if let Some(idx) = self.simulations.iter().position(|s| s.id == scenario_id) {
            let mut completed = self.simulations.remove(idx);
            completed.status = ScenarioStatus::Completed {
                result: result.clone(),
            };
            self.history.push(completed);
        }

        Ok(result)
    }

    /// Create a branch (what-if alternative).
    pub fn create_branch(
        &mut self,
        parent_scenario_id: &str,
        diverge_at_step: u32,
        alternative_action: SimulatedAction,
        remaining_actions: Vec<SimulatedAction>,
    ) -> Result<String, SimulationError> {
        if !self.history.iter().any(|s| s.id == parent_scenario_id) {
            return Err(SimulationError::NotFound(parent_scenario_id.into()));
        }

        let branch = ScenarioBranch {
            branch_id: uuid::Uuid::new_v4().to_string(),
            parent_scenario: parent_scenario_id.into(),
            diverge_at_step,
            alternative_action,
            remaining_actions,
            outcome: None,
        };

        let id = branch.branch_id.clone();
        self.branches.push(branch);
        Ok(id)
    }

    pub fn history(&self) -> &[Scenario] {
        &self.history
    }

    pub fn active_count(&self) -> usize {
        self.simulations.len()
    }

    pub fn get_result(&self, scenario_id: &str) -> Option<&SimulationResult> {
        self.history.iter().find_map(|s| {
            if let ScenarioStatus::Completed { ref result } = s.status {
                if s.id == scenario_id {
                    return Some(result);
                }
            }
            None
        })
    }

    pub fn agent_history(&self, agent_id: &str) -> Vec<&Scenario> {
        self.history
            .iter()
            .filter(|s| s.agent_id == agent_id)
            .collect()
    }
}

/// Assess overall risk from step results.
fn assess_risk(steps: &[StepResult]) -> RiskAssessment {
    let high_risk_count = steps
        .iter()
        .filter(|s| matches!(s.risk, StepRisk::High | StepRisk::Critical))
        .count();
    let medium_risk_count = steps
        .iter()
        .filter(|s| matches!(s.risk, StepRisk::Medium))
        .count();
    let failed_count = steps.iter().filter(|s| !s.success).count();

    let has_destructive = steps.iter().any(|s| {
        s.side_effects.iter().any(|e| {
            matches!(
                e,
                SideEffect::DataLoss { .. } | SideEffect::ServiceDisruption { .. }
            )
        })
    });

    if has_destructive || high_risk_count > 0 {
        RiskAssessment {
            level: RiskLevel::High,
            score: (0.8 + (high_risk_count as f64 * 0.05)).min(1.0),
            factors: vec![
                format!("{} high-risk steps", high_risk_count),
                if has_destructive {
                    "Contains destructive actions".into()
                } else {
                    String::new()
                },
            ]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect(),
            mitigations: vec![
                "Create backup before proceeding".into(),
                "Execute in maintenance window".into(),
                "Have rollback plan ready".into(),
            ],
        }
    } else if medium_risk_count > 0 || failed_count > 0 {
        RiskAssessment {
            level: RiskLevel::Medium,
            score: 0.4 + (medium_risk_count as f64 * 0.1),
            factors: vec![
                format!("{} medium-risk steps", medium_risk_count),
                format!("{} steps failed in simulation", failed_count),
            ]
            .into_iter()
            .filter(|s| !s.starts_with("0 "))
            .collect(),
            mitigations: vec!["Monitor closely during execution".into()],
        }
    } else {
        RiskAssessment {
            level: RiskLevel::Low,
            score: 0.1,
            factors: vec!["All steps low-risk and successful".into()],
            mitigations: Vec::new(),
        }
    }
}

/// Generate a go/no-go recommendation.
fn generate_recommendation(steps: &[StepResult], risk: &RiskAssessment) -> Recommendation {
    let all_succeeded = steps.iter().all(|r| r.success);

    match (&risk.level, all_succeeded) {
        (RiskLevel::Low, true) => Recommendation::Proceed {
            confidence: 0.95,
            note: "All steps simulated successfully with low risk".into(),
        },
        (RiskLevel::Medium, true) => Recommendation::ProceedWithCaution {
            confidence: 0.70,
            precautions: risk.mitigations.clone(),
        },
        (RiskLevel::High, _) => Recommendation::NeedsReview {
            reason: format!("High risk: {}", risk.factors.join(", ")),
        },
        (_, false) => Recommendation::DoNotProceed {
            reason: format!(
                "{} of {} steps failed in simulation",
                steps.iter().filter(|r| !r.success).count(),
                steps.len(),
            ),
        },
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SimulationError {
    #[error("Too many steps: {requested} (max {maximum})")]
    TooManySteps { requested: u32, maximum: u32 },
    #[error("Concurrency limit: {active} active (max {maximum})")]
    ConcurrencyLimit { active: usize, maximum: usize },
    #[error("Scenario not found: {0}")]
    NotFound(String),
    #[error("Sandbox error: {0}")]
    SandboxError(String),
    #[error("Timeout")]
    Timeout,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::*;

    fn make_action(step: u32, action_type: SimActionType) -> SimulatedAction {
        SimulatedAction {
            step,
            action_type,
            description: format!("Step {step}"),
            depends_on: Vec::new(),
            predicted_outcome: None,
        }
    }

    #[test]
    fn test_scenario_creation() {
        let scenario = Scenario::new(
            "agent-1".into(),
            "Test scenario".into(),
            vec![make_action(
                1,
                SimActionType::TerminalCommand {
                    command: "ls -la".into(),
                    working_dir: None,
                },
            )],
        );
        assert_eq!(scenario.agent_id, "agent-1");
        assert_eq!(scenario.actions.len(), 1);
        assert!(matches!(scenario.status, ScenarioStatus::Pending));
    }

    #[test]
    fn test_simulation_max_steps() {
        let config = SimulationConfig {
            max_steps: 2,
            ..Default::default()
        };
        let mut engine = SimulationEngine::new(config);

        let actions: Vec<SimulatedAction> = (1..=5)
            .map(|i| {
                make_action(
                    i,
                    SimActionType::TerminalCommand {
                        command: "echo hi".into(),
                        working_dir: None,
                    },
                )
            })
            .collect();

        let scenario = Scenario::new("a1".into(), "too many".into(), actions);
        let result = engine.submit(scenario);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SimulationError::TooManySteps { .. }
        ));
    }

    #[test]
    fn test_simulation_concurrency_limit() {
        let config = SimulationConfig {
            max_concurrent: 1,
            ..Default::default()
        };
        let mut engine = SimulationEngine::new(config);

        let s1 = Scenario {
            id: "s1".into(),
            agent_id: "a1".into(),
            description: "first".into(),
            actions: vec![make_action(
                1,
                SimActionType::LlmCall {
                    model: "m".into(),
                    prompt: "p".into(),
                },
            )],
            preconditions: Vec::new(),
            expected_outcome: None,
            created_at: 0,
            status: ScenarioStatus::Running, // Already running
        };
        engine.simulations.push(s1);

        let s2 = Scenario::new(
            "a2".into(),
            "second".into(),
            vec![make_action(
                1,
                SimActionType::LlmCall {
                    model: "m".into(),
                    prompt: "p".into(),
                },
            )],
        );
        let result = engine.submit(s2);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SimulationError::ConcurrencyLimit { .. }
        ));
    }

    #[test]
    fn test_sandbox_file_write() {
        let config = SimulationConfig::default();
        let mut sandbox = SimulationSandbox::new(&config);

        let action = make_action(
            1,
            SimActionType::FileWrite {
                path: "/tmp/test.txt".into(),
                content: "hello world".into(),
            },
        );
        let result = sandbox.simulate_action(&action);
        assert!(result.success);
        assert!(sandbox.virtual_fs().contains_key("/tmp/test.txt"));
    }

    #[test]
    fn test_sandbox_file_delete() {
        let config = SimulationConfig::default();
        let mut sandbox = SimulationSandbox::new(&config);

        // Write first
        sandbox.simulate_action(&make_action(
            1,
            SimActionType::FileWrite {
                path: "/tmp/del.txt".into(),
                content: "data".into(),
            },
        ));
        assert!(sandbox.virtual_fs().contains_key("/tmp/del.txt"));

        // Delete
        let result = sandbox.simulate_action(&make_action(
            2,
            SimActionType::FileDelete {
                path: "/tmp/del.txt".into(),
            },
        ));
        assert!(result.success);
        assert!(!sandbox.virtual_fs().contains_key("/tmp/del.txt"));
        assert!(matches!(result.risk, StepRisk::High));
    }

    #[test]
    fn test_sandbox_terminal_analysis() {
        let config = SimulationConfig::default();
        let mut sandbox = SimulationSandbox::new(&config);

        let ls = sandbox.simulate_action(&make_action(
            1,
            SimActionType::TerminalCommand {
                command: "ls -la".into(),
                working_dir: None,
            },
        ));
        assert!(matches!(ls.risk, StepRisk::Low));

        let rm = sandbox.simulate_action(&make_action(
            2,
            SimActionType::TerminalCommand {
                command: "rm -rf /".into(),
                working_dir: None,
            },
        ));
        assert!(matches!(rm.risk, StepRisk::Critical));
    }

    #[test]
    fn test_sandbox_http_risk() {
        let config = SimulationConfig::default();
        let mut sandbox = SimulationSandbox::new(&config);

        let get = sandbox.simulate_action(&make_action(
            1,
            SimActionType::HttpRequest {
                method: "GET".into(),
                url: "https://api.example.com".into(),
                body: None,
            },
        ));
        assert!(matches!(get.risk, StepRisk::Low));

        let delete = sandbox.simulate_action(&make_action(
            2,
            SimActionType::HttpRequest {
                method: "DELETE".into(),
                url: "https://api.example.com/resource".into(),
                body: None,
            },
        ));
        assert!(matches!(delete.risk, StepRisk::High));
    }

    #[test]
    fn test_sandbox_deploy_risk() {
        let config = SimulationConfig::default();
        let mut sandbox = SimulationSandbox::new(&config);

        let result = sandbox.simulate_action(&make_action(
            1,
            SimActionType::Deploy {
                target: "production".into(),
                artifact: "v2.0.0".into(),
            },
        ));
        assert!(matches!(result.risk, StepRisk::High));
        assert!(result
            .side_effects
            .iter()
            .any(|e| matches!(e, SideEffect::ServiceDisruption { .. })));
    }

    #[test]
    fn test_risk_assessment_low() {
        let steps = vec![
            StepResult {
                step: 1,
                success: true,
                output: "ok".into(),
                side_effects: Vec::new(),
                risk: StepRisk::Low,
            },
            StepResult {
                step: 2,
                success: true,
                output: "ok".into(),
                side_effects: Vec::new(),
                risk: StepRisk::Low,
            },
        ];
        let risk = assess_risk(&steps);
        assert_eq!(risk.level, RiskLevel::Low);
    }

    #[test]
    fn test_risk_assessment_high() {
        let steps = vec![
            StepResult {
                step: 1,
                success: true,
                output: "ok".into(),
                side_effects: Vec::new(),
                risk: StepRisk::Low,
            },
            StepResult {
                step: 2,
                success: true,
                output: "danger".into(),
                side_effects: vec![SideEffect::DataLoss {
                    description: "rm".into(),
                }],
                risk: StepRisk::High,
            },
        ];
        let risk = assess_risk(&steps);
        assert_eq!(risk.level, RiskLevel::High);
    }

    #[test]
    fn test_recommendation_proceed() {
        let steps = vec![StepResult {
            step: 1,
            success: true,
            output: "ok".into(),
            side_effects: Vec::new(),
            risk: StepRisk::Low,
        }];
        let risk = assess_risk(&steps);
        let rec = generate_recommendation(&steps, &risk);
        assert!(matches!(rec, Recommendation::Proceed { .. }));
    }

    #[test]
    fn test_recommendation_do_not_proceed() {
        let steps = vec![StepResult {
            step: 1,
            success: false,
            output: "failed".into(),
            side_effects: Vec::new(),
            risk: StepRisk::Low,
        }];
        let risk = assess_risk(&steps);
        let rec = generate_recommendation(&steps, &risk);
        assert!(matches!(rec, Recommendation::DoNotProceed { .. }));
    }

    #[test]
    fn test_precondition_check() {
        let mut engine = SimulationEngine::new(SimulationConfig::default());

        let scenario = Scenario {
            id: "pre-check".into(),
            agent_id: "a1".into(),
            description: "precondition test".into(),
            actions: vec![make_action(
                1,
                SimActionType::TerminalCommand {
                    command: "echo ok".into(),
                    working_dir: None,
                },
            )],
            preconditions: vec![Condition {
                description: "Nonexistent file required".into(),
                check_type: ConditionCheck::FileExists(
                    "/nonexistent/path/that/doesnt/exist/ever".into(),
                ),
            }],
            expected_outcome: None,
            created_at: 0,
            status: ScenarioStatus::Pending,
        };

        let id = engine.submit(scenario).unwrap();
        let result = engine.run_simulation(&id).unwrap();
        assert!(!result.success);
        assert!(matches!(
            result.recommendation,
            Recommendation::DoNotProceed { .. }
        ));
    }

    #[test]
    fn test_dependency_ordering() {
        let mut engine = SimulationEngine::new(SimulationConfig::default());

        let scenario = Scenario::new(
            "a1".into(),
            "dependency test".into(),
            vec![
                // Step 2 depends on step 1, but step 1 will fail
                SimulatedAction {
                    step: 1,
                    action_type: SimActionType::FileDelete {
                        path: "/nonexistent/file/zzz".into(),
                    },
                    description: "Delete nonexistent".into(),
                    depends_on: Vec::new(),
                    predicted_outcome: None,
                },
                SimulatedAction {
                    step: 2,
                    action_type: SimActionType::TerminalCommand {
                        command: "echo done".into(),
                        working_dir: None,
                    },
                    description: "Depends on step 1".into(),
                    depends_on: vec![1],
                    predicted_outcome: None,
                },
            ],
        );

        let id = engine.submit(scenario).unwrap();
        let result = engine.run_simulation(&id).unwrap();

        // Step 1 fails (file not found), step 2 skips (dependency not met)
        assert!(!result.success);
        assert_eq!(result.step_results.len(), 2);
        assert!(!result.step_results[0].success); // delete failed
        assert!(!result.step_results[1].success); // dependency not met
    }
}
