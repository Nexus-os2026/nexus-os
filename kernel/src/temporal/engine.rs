//! Temporal Engine — fork reality, simulate parallel futures, commit the best one.

use crate::consciousness::ConsciousnessState;
use crate::manifest::AgentManifest;
use crate::temporal::types::{
    EvalStrategy, ForkStatus, TemporalConfig, TemporalDecision, TemporalError, TimelineFork,
    TimelineStep,
};
use serde_json::Value;
use std::time::Instant;

/// The temporal engine: forks decisions into parallel timelines, simulates
/// each, scores them, and selects the best future.
#[derive(Debug, Clone)]
pub struct TemporalEngine {
    config: TemporalConfig,
    /// History of all decisions made through this engine.
    history: Vec<TemporalDecision>,
}

impl Default for TemporalEngine {
    fn default() -> Self {
        Self::new(TemporalConfig::default())
    }
}

impl TemporalEngine {
    pub fn new(config: TemporalConfig) -> Self {
        Self {
            config,
            history: Vec::new(),
        }
    }

    pub fn config(&self) -> &TemporalConfig {
        &self.config
    }

    pub fn update_config(&mut self, config: TemporalConfig) {
        self.config = config;
    }

    pub fn history(&self) -> &[TemporalDecision] {
        &self.history
    }

    // -----------------------------------------------------------------------
    // Fork count calculation
    // -----------------------------------------------------------------------

    /// Decide how many forks to create based on the agent's consciousness state.
    ///
    /// - High urgency → fewer forks (fast decision).
    /// - Low confidence → more forks (explore options).
    /// - Default → 3 forks.
    pub fn calculate_fork_count(&self, consciousness: &ConsciousnessState) -> u32 {
        let base = self.config.max_parallel_forks;
        if consciousness.urgency > 0.8 {
            2_u32.min(base)
        } else if consciousness.confidence < 0.3 {
            base
        } else {
            3_u32.min(base)
        }
    }

    // -----------------------------------------------------------------------
    // Core fork-and-evaluate (LLM-backed)
    // -----------------------------------------------------------------------

    /// Fork reality: generate N different approaches via LLM, simulate each,
    /// score them, and select the best timeline.
    ///
    /// `llm_query` is a closure that sends a prompt to the LLM and returns
    /// `(response_text, tokens_used)`.  This keeps the engine decoupled from
    /// the concrete provider.
    pub fn fork_and_evaluate<F>(
        &mut self,
        request: &str,
        agent: &AgentManifest,
        consciousness: &ConsciousnessState,
        mut llm_query: F,
    ) -> Result<TemporalDecision, TemporalError>
    where
        F: FnMut(&str) -> Result<(String, u32), TemporalError>,
    {
        let start = Instant::now();
        let fork_count = self.calculate_fork_count(consciousness);

        if fork_count == 0 {
            return Err(TemporalError::InvalidForkCount(0));
        }

        let branch_point = format!("Agent '{}' deciding on: {}", agent.name, request);
        let mut decision = TemporalDecision::new(request, &branch_point);
        let mut total_tokens: u64 = 0;

        // --- Step 1: Generate N approaches via LLM ---
        let gen_prompt = format!(
            "Given this task: {request}\n\
             Generate {fork_count} fundamentally different approaches.\n\
             For each approach, return a JSON array of objects with fields:\n\
             - \"name\": short approach name\n\
             - \"strategy\": one-sentence description\n\
             - \"steps\": array of 3-5 action strings\n\
             - \"risk\": estimated risk 0.0-1.0\n\
             Return ONLY the JSON array, no markdown."
        );

        let (approaches_raw, gen_tokens) = llm_query(&gen_prompt)?;
        total_tokens += gen_tokens as u64;

        if total_tokens > self.config.fork_budget_tokens {
            return Err(TemporalError::BudgetExhausted {
                used: total_tokens,
                limit: self.config.fork_budget_tokens,
            });
        }

        let approaches = parse_approaches(&approaches_raw, fork_count)?;

        // --- Step 2: Simulate each approach ---
        let max_depth = self.config.max_depth_per_fork;
        for approach in &approaches {
            let mut fork = TimelineFork::new(&branch_point, &approach.name);

            let steps_to_sim: Vec<&str> = approach
                .steps
                .iter()
                .take(max_depth as usize)
                .map(String::as_str)
                .collect();

            for (i, action) in steps_to_sim.iter().enumerate() {
                // Budget gate
                if total_tokens > self.config.fork_budget_tokens {
                    break;
                }

                let sim_prompt = format!(
                    "You chose approach: {}\n\
                     Step {}: {action}\n\
                     What is the likely outcome? Score quality 0-10.\n\
                     What side effects or risks emerge?\n\
                     Return JSON: {{\"outcome\": \"...\", \"score\": N, \
                     \"side_effects\": [\"...\"], \"reversible\": true/false}}\n\
                     Return ONLY the JSON, no markdown.",
                    approach.name,
                    i + 1
                );

                match llm_query(&sim_prompt) {
                    Ok((sim_raw, sim_tokens)) => {
                        total_tokens += sim_tokens as u64;
                        let step =
                            parse_step(&sim_raw, (i + 1) as u32, action).unwrap_or(TimelineStep {
                                step_number: (i + 1) as u32,
                                action: action.to_string(),
                                simulated_outcome: sim_raw.clone(),
                                score: 5.0,
                                side_effects: vec![],
                                reversible: true,
                            });
                        fork.steps.push(step);
                    }
                    Err(_) => {
                        // Record a low-score step on LLM failure so the fork
                        // is penalised but not dropped entirely.
                        fork.steps.push(TimelineStep {
                            step_number: (i + 1) as u32,
                            action: action.to_string(),
                            simulated_outcome: "simulation failed".into(),
                            score: 1.0,
                            side_effects: vec!["LLM error during simulation".into()],
                            reversible: true,
                        });
                    }
                }
            }

            fork.recalculate_scores();
            fork.status = ForkStatus::Completed;
            decision.forks.push(fork);
        }

        // --- Step 3: Select best timeline ---
        self.select_best_fork(&mut decision);

        decision.total_tokens_used = total_tokens;
        decision.simulation_time_ms = start.elapsed().as_millis() as u64;

        self.history.push(decision.clone());
        Ok(decision)
    }

    // -----------------------------------------------------------------------
    // Fork selection
    // -----------------------------------------------------------------------

    /// Apply the configured evaluation strategy to select the best fork.
    pub fn select_best_fork(&self, decision: &mut TemporalDecision) {
        if decision.forks.is_empty() {
            return;
        }

        let winner_idx = match self.config.evaluation_strategy {
            EvalStrategy::BestFinalScore => decision
                .forks
                .iter()
                .enumerate()
                .max_by(|a, b| {
                    a.1.final_score
                        .partial_cmp(&b.1.final_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i),
            EvalStrategy::BestAverageScore => decision
                .forks
                .iter()
                .enumerate()
                .max_by(|a, b| {
                    a.1.average_score()
                        .partial_cmp(&b.1.average_score())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i),
            EvalStrategy::LowestRisk => decision
                .forks
                .iter()
                .enumerate()
                .max_by(|a, b| {
                    a.1.risk_score
                        .partial_cmp(&b.1.risk_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i),
            EvalStrategy::UserChoice => {
                // Don't auto-select; leave for manual selection.
                decision.reasoning =
                    "UserChoice strategy: awaiting user selection of timeline.".into();
                return;
            }
        };

        if let Some(idx) = winner_idx {
            let fork_id = decision.forks[idx].fork_id.clone();
            for (i, fork) in decision.forks.iter_mut().enumerate() {
                if i == idx {
                    fork.status = ForkStatus::Selected;
                } else {
                    fork.status = ForkStatus::Pruned;
                }
            }
            decision.selected_fork = Some(fork_id);
            decision.reasoning = format!(
                "Selected by {:?}: fork '{}' with final_score={:.1}, risk_score={:.1}, avg={:.1}",
                self.config.evaluation_strategy,
                decision.forks[idx].chosen_action,
                decision.forks[idx].final_score,
                decision.forks[idx].risk_score,
                decision.forks[idx].average_score(),
            );
        }
    }

    /// Manually select a fork (for UserChoice strategy or override).
    pub fn manual_select_fork(
        &mut self,
        decision_id: &str,
        fork_id: &str,
    ) -> Result<(), TemporalError> {
        let decision = self
            .history
            .iter_mut()
            .find(|d| d.decision_id == decision_id)
            .ok_or_else(|| TemporalError::NotFound(decision_id.to_string()))?;

        let fork_exists = decision.forks.iter().any(|f| f.fork_id == fork_id);
        if !fork_exists {
            return Err(TemporalError::NotFound(fork_id.to_string()));
        }

        for fork in &mut decision.forks {
            if fork.fork_id == fork_id {
                fork.status = ForkStatus::Selected;
            } else {
                fork.status = ForkStatus::Pruned;
            }
        }
        decision.selected_fork = Some(fork_id.to_string());
        decision.reasoning = format!("Manually selected fork {fork_id}");
        Ok(())
    }

    /// Get a specific decision by ID.
    pub fn get_decision(&self, decision_id: &str) -> Option<&TemporalDecision> {
        self.history.iter().find(|d| d.decision_id == decision_id)
    }
}

// ---------------------------------------------------------------------------
// LLM response parsing helpers
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Approach {
    name: String,
    #[allow(dead_code)]
    strategy: String,
    steps: Vec<String>,
    #[allow(dead_code)]
    risk: f64,
}

fn parse_approaches(raw: &str, expected: u32) -> Result<Vec<Approach>, TemporalError> {
    // Try to parse as JSON array.  Fall back to synthetic approaches on failure.
    let trimmed = raw.trim();
    // Strip markdown fences if present
    let json_str = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };

    match serde_json::from_str::<Vec<Value>>(json_str) {
        Ok(arr) => {
            let mut approaches = Vec::new();
            for item in arr.iter().take(expected as usize) {
                let name = item["name"].as_str().unwrap_or("approach").to_string();
                let strategy = item["strategy"].as_str().unwrap_or("").to_string();
                let steps: Vec<String> = item["steps"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_else(|| vec!["execute task".into()]);
                let risk = item["risk"].as_f64().unwrap_or(0.5);
                approaches.push(Approach {
                    name,
                    strategy,
                    steps,
                    risk,
                });
            }
            if approaches.is_empty() {
                return Err(TemporalError::ParseError(
                    "LLM returned empty approaches array".into(),
                ));
            }
            Ok(approaches)
        }
        Err(_) => {
            // Fallback: generate synthetic approaches from the raw text.
            let mut approaches = Vec::new();
            for i in 0..expected {
                approaches.push(Approach {
                    name: format!("approach-{}", i + 1),
                    strategy: raw.chars().take(200).collect(),
                    steps: vec!["execute task".into()],
                    risk: 0.5,
                });
            }
            Ok(approaches)
        }
    }
}

fn parse_step(raw: &str, step_number: u32, action: &str) -> Result<TimelineStep, TemporalError> {
    let trimmed = raw.trim();
    let json_str = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };

    let val: Value =
        serde_json::from_str(json_str).map_err(|e| TemporalError::ParseError(e.to_string()))?;

    Ok(TimelineStep {
        step_number,
        action: action.to_string(),
        simulated_outcome: val["outcome"].as_str().unwrap_or("unknown").to_string(),
        score: val["score"].as_f64().unwrap_or(5.0).clamp(0.0, 10.0),
        side_effects: val["side_effects"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        reversible: val["reversible"].as_bool().unwrap_or(true),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consciousness::ConsciousnessState;
    use crate::manifest::AgentManifest;

    fn test_manifest() -> AgentManifest {
        AgentManifest {
            name: "test-agent".into(),
            version: "1.0.0".into(),
            capabilities: vec!["llm.query".into()],
            fuel_budget: 1000,
            autonomy_level: Some(2),
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
        }
    }

    #[test]
    fn fork_count_high_urgency() {
        let engine = TemporalEngine::default();
        let mut c = ConsciousnessState::new("a");
        c.urgency = 0.9;
        assert_eq!(engine.calculate_fork_count(&c), 2);
    }

    #[test]
    fn fork_count_low_confidence() {
        let engine = TemporalEngine::default();
        let mut c = ConsciousnessState::new("a");
        c.confidence = 0.2;
        c.urgency = 0.3;
        assert_eq!(engine.calculate_fork_count(&c), 5);
    }

    #[test]
    fn fork_count_default() {
        let engine = TemporalEngine::default();
        let c = ConsciousnessState::new("a");
        // default confidence=0.5, urgency=0.0
        assert_eq!(engine.calculate_fork_count(&c), 3);
    }

    #[test]
    fn fork_and_evaluate_basic() {
        let mut engine = TemporalEngine::default();
        let agent = test_manifest();
        let mut cons = ConsciousnessState::new("a");
        cons.urgency = 0.9; // 2 forks

        let mut call_count = 0;
        let mock_llm = |prompt: &str| -> Result<(String, u32), TemporalError> {
            call_count += 1;
            if prompt.contains("Generate") {
                Ok((
                    r#"[
                        {"name":"fast","strategy":"quick","steps":["step1","step2"],"risk":0.2},
                        {"name":"safe","strategy":"careful","steps":["step1","step2"],"risk":0.1}
                    ]"#
                    .into(),
                    100,
                ))
            } else {
                Ok((
                    r#"{"outcome":"success","score":8.0,"side_effects":[],"reversible":true}"#
                        .into(),
                    50,
                ))
            }
        };

        let decision = engine
            .fork_and_evaluate("design schema", &agent, &cons, mock_llm)
            .unwrap();
        assert_eq!(decision.forks.len(), 2);
        assert!(decision.selected_fork.is_some());
        assert!(decision.total_tokens_used > 0);
        assert_eq!(engine.history().len(), 1);
    }

    #[test]
    fn fork_and_evaluate_budget_exhausted() {
        let config = TemporalConfig {
            fork_budget_tokens: 10, // very small
            ..TemporalConfig::default()
        };
        let mut engine = TemporalEngine::new(config);
        let agent = test_manifest();
        let cons = ConsciousnessState::new("a");

        let mock_llm = |_prompt: &str| -> Result<(String, u32), TemporalError> {
            Ok(("[]".into(), 100)) // 100 tokens > budget of 10
        };

        let result = engine.fork_and_evaluate("task", &agent, &cons, mock_llm);
        assert!(matches!(result, Err(TemporalError::BudgetExhausted { .. })));
    }

    #[test]
    fn manual_select_fork() {
        let mut engine = TemporalEngine::default();
        let agent = test_manifest();
        let mut cons = ConsciousnessState::new("a");
        cons.urgency = 0.9;

        let config = TemporalConfig {
            evaluation_strategy: EvalStrategy::UserChoice,
            ..TemporalConfig::default()
        };
        engine.update_config(config);

        let mock_llm = |prompt: &str| -> Result<(String, u32), TemporalError> {
            if prompt.contains("Generate") {
                Ok((
                    r#"[
                        {"name":"A","strategy":"s","steps":["s1"],"risk":0.2},
                        {"name":"B","strategy":"s","steps":["s1"],"risk":0.1}
                    ]"#
                    .into(),
                    100,
                ))
            } else {
                Ok((
                    r#"{"outcome":"ok","score":7.0,"side_effects":[],"reversible":true}"#.into(),
                    50,
                ))
            }
        };

        let decision = engine
            .fork_and_evaluate("task", &agent, &cons, mock_llm)
            .unwrap();
        assert!(decision.selected_fork.is_none()); // UserChoice: not auto-selected

        let fork_id = decision.forks[1].fork_id.clone();
        let decision_id = decision.decision_id.clone();
        engine.manual_select_fork(&decision_id, &fork_id).unwrap();

        let updated = engine.get_decision(&decision_id).unwrap();
        assert_eq!(updated.selected_fork.as_deref(), Some(fork_id.as_str()));
    }

    #[test]
    fn select_lowest_risk() {
        let config = TemporalConfig {
            evaluation_strategy: EvalStrategy::LowestRisk,
            ..TemporalConfig::default()
        };
        let engine = TemporalEngine::new(config);

        let mut decision = TemporalDecision::new("task", "bp");
        let mut f1 = TimelineFork::new("bp", "risky");
        f1.steps.push(TimelineStep {
            step_number: 1,
            action: "a".into(),
            simulated_outcome: "ok".into(),
            score: 3.0,
            side_effects: vec![],
            reversible: true,
        });
        f1.recalculate_scores();
        f1.status = ForkStatus::Completed;

        let mut f2 = TimelineFork::new("bp", "safe");
        f2.steps.push(TimelineStep {
            step_number: 1,
            action: "b".into(),
            simulated_outcome: "ok".into(),
            score: 7.0,
            side_effects: vec![],
            reversible: true,
        });
        f2.recalculate_scores();
        f2.status = ForkStatus::Completed;

        decision.forks.push(f1);
        decision.forks.push(f2);

        engine.select_best_fork(&mut decision);
        // LowestRisk picks highest min-step score → f2 (risk_score=7.0 > 3.0)
        assert_eq!(
            decision.selected_fork.as_deref(),
            Some(decision.forks[1].fork_id.as_str())
        );
    }

    #[test]
    fn parse_approaches_valid_json() {
        let raw = r#"[{"name":"A","strategy":"s","steps":["s1","s2"],"risk":0.3}]"#;
        let approaches = parse_approaches(raw, 3).unwrap();
        assert_eq!(approaches.len(), 1);
        assert_eq!(approaches[0].name, "A");
        assert_eq!(approaches[0].steps.len(), 2);
    }

    #[test]
    fn parse_approaches_fallback() {
        let raw = "not json at all";
        let approaches = parse_approaches(raw, 3).unwrap();
        assert_eq!(approaches.len(), 3);
        assert_eq!(approaches[0].name, "approach-1");
    }

    #[test]
    fn parse_step_valid() {
        let raw = r#"{"outcome":"deployed","score":8.5,"side_effects":["latency spike"],"reversible":false}"#;
        let step = parse_step(raw, 1, "deploy").unwrap();
        assert_eq!(step.simulated_outcome, "deployed");
        assert!((step.score - 8.5).abs() < f64::EPSILON);
        assert!(!step.reversible);
        assert_eq!(step.side_effects, vec!["latency spike"]);
    }

    #[test]
    fn parse_step_with_markdown_fences() {
        let raw = "```json\n{\"outcome\":\"ok\",\"score\":6.0,\"side_effects\":[],\"reversible\":true}\n```";
        let step = parse_step(raw, 2, "test").unwrap();
        assert!((step.score - 6.0).abs() < f64::EPSILON);
    }
}
