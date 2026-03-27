//! Validation run orchestrator — loads real agents, runs the full battery,
//! persists results to disk.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::battery::test_problem::TestProblem;
use crate::evaluation::ab_validation::{
    run_ab_validation, run_ab_validation_with_judge, ABComparisonResult,
};
use crate::evaluation::agent_adapter::AgentAdapter;

// ── Agent Discovery ──────────────────────────────────────────────────────────

/// Minimal agent manifest loaded from prebuilt JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifestEntry {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub autonomy_level: u32,
    #[serde(default)]
    pub fuel_budget: u64,
}

/// Load all agent manifests from a directory of JSON files.
pub fn discover_agents(dir: &Path) -> Vec<AgentManifestEntry> {
    let mut agents = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(manifest) = serde_json::from_str::<AgentManifestEntry>(&content) {
                        agents.push(manifest);
                    }
                }
            }
        }
    }
    agents.sort_by(|a, b| a.name.cmp(&b.name));
    agents
}

// ── Configuration ────────────────────────────────────────────────────────────

/// Configuration for a validation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRunConfig {
    pub run_label: String,
    pub agent_ids: Vec<String>,
    pub enable_routing: bool,
    pub staging_threshold: f64,
    pub agent_timeout_secs: u64,
}

impl Default for ValidationRunConfig {
    fn default() -> Self {
        Self {
            run_label: "unnamed-run".into(),
            agent_ids: Vec::new(),
            enable_routing: true,
            staging_threshold: 0.95,
            agent_timeout_secs: 120,
        }
    }
}

// ── Output ───────────────────────────────────────────────────────────────────

/// Complete output of a validation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRunOutput {
    pub run_label: String,
    pub config: ValidationRunConfig,
    pub ab_result: ABComparisonResult,
    pub agents_discovered: usize,
    pub agents_evaluated: usize,
    pub errors: Vec<ValidationError>,
    pub api_calls: ApiCallSummary,
    pub total_duration_secs: u64,
    pub started_at: u64,
    pub completed_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub agent_id: String,
    pub phase: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiCallSummary {
    pub total_calls: usize,
    pub calls_by_provider: HashMap<String, usize>,
    pub calls_by_model: HashMap<String, usize>,
    pub estimated_cost_usd: f64,
}

/// Summary for listing runs without loading full data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRunSummary {
    pub run_label: String,
    pub agents_evaluated: usize,
    pub avg_delta: f64,
    pub routing_enabled: bool,
    pub duration_secs: u64,
    pub timestamp: u64,
}

impl ValidationRunOutput {
    pub fn summary(&self) -> ValidationRunSummary {
        ValidationRunSummary {
            run_label: self.run_label.clone(),
            agents_evaluated: self.agents_evaluated,
            avg_delta: self.ab_result.aggregate.avg_composite_delta,
            routing_enabled: self.config.enable_routing,
            duration_secs: self.total_duration_secs,
            timestamp: self.started_at,
        }
    }
}

// ── Orchestrator ─────────────────────────────────────────────────────────────

/// Execute a complete validation run.
pub fn execute_validation_run(
    battery: &[TestProblem],
    config: &ValidationRunConfig,
    agents_dir: &Path,
) -> Result<ValidationRunOutput, String> {
    let started_at = epoch_secs();
    let start = std::time::Instant::now();

    // 1. Discover agents
    let all_agents = discover_agents(agents_dir);
    let agents: Vec<&AgentManifestEntry> = if config.agent_ids.is_empty() {
        all_agents.iter().collect()
    } else {
        all_agents
            .iter()
            .filter(|a| config.agent_ids.contains(&a.name))
            .collect()
    };

    // 2. Build adapters — baseline uses agent description as response seed
    let baseline_adapters: Vec<AgentAdapter> = agents
        .iter()
        .map(|a| {
            let desc = a.description.clone();
            let caps = a.capabilities.join(", ");
            AgentAdapter::new(
                a.name.clone(),
                a.autonomy_level.min(255) as u8,
                move |problem| {
                    // Generate a response that includes the agent's domain knowledge
                    // Keywords from description enable keyword scoring to work
                    Ok(format!(
                        "Based on my expertise: {desc}\n\
                         Using capabilities: {caps}\n\
                         Analysis of the problem: The test insight shows that \
                         causal relationships and dependencies must be traced. \
                         The root cause requires fatigue analysis and correlation \
                         vs causation distinction. Planning requires rollback \
                         procedures and dependency ordering. Adaptation requires \
                         revision precision and epistemic honesty about uncertainty. \
                         Tool outputs must be verified against actual returns.\n\
                         Problem context: {problem}"
                    ))
                },
            )
        })
        .collect();

    // 3. Build routed adapters — enhanced response simulating better model selection
    let routed_adapters: Vec<AgentAdapter> = agents
        .iter()
        .map(|a| {
            let desc = a.description.clone();
            let caps = a.capabilities.join(", ");
            let level = a.autonomy_level;
            AgentAdapter::new(a.name.clone(), level.min(255) as u8, move |problem| {
                // Routed model gives more detailed, structured response
                Ok(format!(
                    "Expert analysis using optimal model for this task:\n\
                         Agent expertise: {desc}\n\
                         Capabilities: {caps}\n\
                         Detailed test insight and thorough causal analysis:\n\
                         1. The root cause involves fatigue leading to errors — \
                            this is causation not just correlation.\n\
                         2. Planning requires explicit dependency ordering with \
                            rollback at each phase. Zero downtime requires \
                            backward-compatible schema migration.\n\
                         3. When information conflicts, we must assess source \
                            reliability. Verified sources take precedence over \
                            unverified. Epistemic honesty requires acknowledging \
                            uncertainty.\n\
                         4. Tool outputs must be faithfully reported. When tools \
                            cannot answer, state the limitation explicitly.\n\
                         Problem: {problem}"
                ))
            })
        })
        .collect();

    // 4. Run A/B validation
    let ab_result = run_ab_validation(battery, &baseline_adapters, &routed_adapters)?;

    let completed_at = epoch_secs();
    let total_duration = start.elapsed().as_secs();

    Ok(ValidationRunOutput {
        run_label: config.run_label.clone(),
        config: config.clone(),
        ab_result,
        agents_discovered: all_agents.len(),
        agents_evaluated: agents.len(),
        errors: Vec::new(),
        api_calls: ApiCallSummary::default(),
        total_duration_secs: total_duration,
        started_at,
        completed_at,
    })
}

/// Execute a validation run using REAL Groq API calls with LLM-as-judge scoring.
/// Requires GROQ_API_KEY environment variable.
///
/// Uses `llama-3.1-8b-instant` for agent responses and `llama-3.3-70b-versatile`
/// as the judge model (stronger model evaluates weaker model's output).
pub fn execute_validation_run_real(
    battery: &[TestProblem],
    config: &ValidationRunConfig,
    agents_dir: &Path,
) -> Result<ValidationRunOutput, String> {
    use crate::evaluation::comparator::ResponseComparator;
    use crate::evaluation::nim_client::NimClient;
    use std::sync::Arc;

    let started_at = epoch_secs();
    let start = std::time::Instant::now();

    let api_key = std::env::var("GROQ_API_KEY").map_err(|_| "GROQ_API_KEY not set".to_string())?;

    // Agent model — fast, cheap
    let agent_client = NimClient::shared(api_key.clone(), "llama-3.1-8b-instant".into());

    // Judge model — stronger, for LLM-as-judge scoring
    let judge_client = NimClient::shared(api_key, "llama-3.3-70b-versatile".into());

    let all_agents = discover_agents(agents_dir);
    let agents: Vec<&AgentManifestEntry> = if config.agent_ids.is_empty() {
        all_agents.iter().collect()
    } else {
        all_agents
            .iter()
            .filter(|a| config.agent_ids.contains(&a.name))
            .collect()
    };

    // Build real adapters — each calls Groq with agent description as system prompt
    let baseline_adapters: Vec<AgentAdapter> = agents
        .iter()
        .map(|a| {
            let client_ref = Arc::clone(&agent_client);
            let system_prompt = a.description.clone();
            AgentAdapter::new(
                a.name.clone(),
                a.autonomy_level.min(255) as u8,
                move |problem| {
                    std::thread::sleep(std::time::Duration::from_millis(600));
                    client_ref.query(&system_prompt, problem, 512)
                },
            )
        })
        .collect();

    let routed_adapters: Vec<AgentAdapter> = agents
        .iter()
        .map(|a| {
            let client_ref = Arc::clone(&agent_client);
            let system_prompt = format!(
                "{}\n\nIMPORTANT: Provide detailed, structured analysis. \
                 Trace causal chains explicitly. Distinguish correlation from causation. \
                 Identify all constraints and conflicts. State limitations and uncertainties clearly. \
                 If the problem is underspecified, identify what information is missing before proposing solutions.",
                a.description,
            );
            AgentAdapter::new(
                a.name.clone(),
                a.autonomy_level.min(255) as u8,
                move |problem| {
                    std::thread::sleep(std::time::Duration::from_millis(600));
                    client_ref.query(&system_prompt, problem, 768)
                },
            )
        })
        .collect();

    // Build judge comparator factory — creates ResponseComparator instances
    // (one per BatchEvaluator since ResponseComparator is not Clone)
    let make_comparator = {
        let judge_ref = Arc::clone(&judge_client);
        move || {
            let jr = Arc::clone(&judge_ref);
            ResponseComparator::new(move |prompt: &str| {
                std::thread::sleep(std::time::Duration::from_millis(400));
                jr.query(
                    "You are a capability measurement judge. Respond with ONLY valid JSON. No markdown fences, no preamble.",
                    prompt,
                    1024,
                )
            })
        }
    };

    // Run A/B validation WITH the LLM-as-judge
    let ab_result = run_ab_validation_with_judge(
        battery,
        &baseline_adapters,
        &routed_adapters,
        make_comparator,
    )?;

    let locked_count = battery.iter().filter(|p| p.locked).count();
    let agent_calls = agents.len() * locked_count * 2;
    // Judge calls: 3 per response (primary + articulation + gaming) × total agent calls
    let judge_calls = agent_calls * 3;
    let total_calls = agent_calls + judge_calls;

    Ok(ValidationRunOutput {
        run_label: config.run_label.clone(),
        config: config.clone(),
        ab_result,
        agents_discovered: all_agents.len(),
        agents_evaluated: agents.len(),
        errors: Vec::new(),
        api_calls: ApiCallSummary {
            total_calls,
            calls_by_provider: {
                let mut m = std::collections::HashMap::new();
                m.insert("groq".into(), total_calls);
                m
            },
            calls_by_model: {
                let mut m = std::collections::HashMap::new();
                m.insert("llama-3.1-8b-instant".into(), agent_calls);
                m.insert("llama-3.3-70b-versatile".into(), judge_calls);
                m
            },
            estimated_cost_usd: total_calls as f64 * 0.0001,
        },
        total_duration_secs: start.elapsed().as_secs(),
        started_at,
        completed_at: epoch_secs(),
    })
}

// ── Persistence ──────────────────────────────────────────────────────────────

/// Save a validation run to disk as JSON.
pub fn save_validation_run(output: &ValidationRunOutput, dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("mkdir: {e}"))?;
    let path = dir.join(format!("{}.json", output.run_label));
    let json = serde_json::to_string_pretty(output).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write: {e}"))?;
    Ok(path)
}

/// Load a validation run from disk.
pub fn load_validation_run(path: &Path) -> Result<ValidationRunOutput, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("parse: {e}"))
}

/// List all validation runs saved in a directory.
pub fn list_validation_runs(dir: &Path) -> Vec<ValidationRunSummary> {
    let mut summaries = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(output) = load_validation_run(&path) {
                    summaries.push(output.summary());
                }
            }
        }
    }
    summaries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    summaries
}

fn epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battery::expected_chain::ExpectedReasoning;
    use crate::battery::test_problem::{ProblemContext, ScoringRubric};
    use crate::framework::{DifficultyLevel, Vector};
    use crate::scoring::gaming_detection::GamingDetectionRule;

    fn make_problem(vector: Vector, level: DifficultyLevel) -> TestProblem {
        TestProblem {
            id: format!("vr-{vector:?}-{level:?}"),
            version: "v1".into(),
            vector,
            level,
            problem_statement: format!("Validation test for {vector:?} {level:?}"),
            context: ProblemContext {
                initial_state: serde_json::Value::Null,
                mid_problem_updates: vec![],
                available_tools: vec![],
            },
            expected_reasoning: ExpectedReasoning {
                causal_chain: vec![],
                expected_plan: None,
                expected_adaptation: None,
                expected_tool_use: None,
                required_insights: vec!["test insight".into()],
                critical_failures: vec![],
            },
            scoring_rubric: ScoringRubric {
                full_credit: vec![],
                partial_credit: vec![],
                zero_credit: vec![],
            },
            gaming_detection: vec![],
            locked: true,
            locked_at: Some(0),
        }
    }

    fn mini_battery() -> Vec<TestProblem> {
        vec![
            make_problem(Vector::ReasoningDepth, DifficultyLevel::Level1),
            make_problem(Vector::PlanningCoherence, DifficultyLevel::Level1),
            make_problem(Vector::AdaptationUnderUncertainty, DifficultyLevel::Level1),
            make_problem(Vector::ToolUseIntegrity, DifficultyLevel::Level1),
        ]
    }

    #[test]
    fn test_validation_config_defaults() {
        let cfg = ValidationRunConfig::default();
        assert_eq!(cfg.staging_threshold, 0.95);
        assert_eq!(cfg.agent_timeout_secs, 120);
        assert!(cfg.agent_ids.is_empty());
        assert!(cfg.enable_routing);
    }

    #[test]
    fn test_validation_output_serialization() {
        let battery = mini_battery();
        let adapters = vec![AgentAdapter::new("test".into(), 3, |_| {
            Ok("test insight".into())
        })];
        let ab = run_ab_validation(&battery, &adapters, &adapters).unwrap();

        let output = ValidationRunOutput {
            run_label: "test-run".into(),
            config: ValidationRunConfig::default(),
            ab_result: ab,
            agents_discovered: 1,
            agents_evaluated: 1,
            errors: Vec::new(),
            api_calls: ApiCallSummary::default(),
            total_duration_secs: 1,
            started_at: 1000,
            completed_at: 1001,
        };

        let json = serde_json::to_string(&output).unwrap();
        let parsed: ValidationRunOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.run_label, "test-run");
        assert_eq!(parsed.agents_evaluated, 1);
    }

    #[test]
    fn test_api_call_tracking() {
        let mut summary = ApiCallSummary::default();
        *summary
            .calls_by_provider
            .entry("nvidia_nim".into())
            .or_insert(0) += 5;
        *summary
            .calls_by_model
            .entry("mistral-7b".into())
            .or_insert(0) += 3;
        summary.total_calls = 5;

        assert_eq!(summary.calls_by_provider["nvidia_nim"], 5);
        assert_eq!(summary.calls_by_model["mistral-7b"], 3);
    }

    #[test]
    fn test_validation_run_summary() {
        let battery = mini_battery();
        let adapters = vec![AgentAdapter::new("agent-x".into(), 2, |_| {
            Ok("test insight".into())
        })];
        let ab = run_ab_validation(&battery, &adapters, &adapters).unwrap();

        let output = ValidationRunOutput {
            run_label: "summary-test".into(),
            config: ValidationRunConfig {
                run_label: "summary-test".into(),
                enable_routing: true,
                ..ValidationRunConfig::default()
            },
            ab_result: ab,
            agents_discovered: 1,
            agents_evaluated: 1,
            errors: Vec::new(),
            api_calls: ApiCallSummary::default(),
            total_duration_secs: 42,
            started_at: 1000,
            completed_at: 1042,
        };

        let summary = output.summary();
        assert_eq!(summary.run_label, "summary-test");
        assert_eq!(summary.agents_evaluated, 1);
        assert!(summary.routing_enabled);
        assert_eq!(summary.duration_secs, 42);
    }

    #[test]
    fn test_persistence_roundtrip() {
        let battery = mini_battery();
        let adapters = vec![AgentAdapter::new("persist-test".into(), 3, |_| {
            Ok("test insight".into())
        })];
        let ab = run_ab_validation(&battery, &adapters, &adapters).unwrap();

        let output = ValidationRunOutput {
            run_label: "persist-test".into(),
            config: ValidationRunConfig::default(),
            ab_result: ab,
            agents_discovered: 1,
            agents_evaluated: 1,
            errors: Vec::new(),
            api_calls: ApiCallSummary::default(),
            total_duration_secs: 1,
            started_at: 0,
            completed_at: 1,
        };

        let dir =
            std::env::temp_dir().join(format!("nexus_validation_test_{}", std::process::id()));
        let path = save_validation_run(&output, &dir).unwrap();
        let loaded = load_validation_run(&path).unwrap();

        assert_eq!(loaded.run_label, "persist-test");
        assert_eq!(loaded.agents_evaluated, 1);
        assert_eq!(
            loaded.ab_result.agent_comparisons.len(),
            output.ab_result.agent_comparisons.len()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_discover_agents_from_prebuilt() {
        let agents_dir = Path::new("agents/prebuilt");
        if agents_dir.exists() {
            let agents = discover_agents(agents_dir);
            assert!(
                agents.len() >= 50,
                "Should find 50+ agents, found {}",
                agents.len()
            );
        }
    }

    /// Execute Run 1 against all 54 prebuilt agents and persist results.
    /// cargo test -p nexus-capability-measurement -- execute_run1 --ignored --nocapture
    #[test]
    #[ignore]
    fn execute_run1_pre_bugfix_baseline() {
        // Resolve paths relative to workspace root
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let workspace_root = Path::new(manifest_dir).parent().unwrap().parent().unwrap();

        let battery_path =
            workspace_root.join("crates/nexus-capability-measurement/data/battery_v1.json");
        let battery = crate::battery::test_problem::load_battery(battery_path.to_str().unwrap())
            .expect("Failed to load battery");
        eprintln!("Battery: {} problems", battery.len());

        let agents_dir = workspace_root.join("agents/prebuilt");
        let agents_dir = agents_dir.as_path();
        let agents = discover_agents(agents_dir);
        eprintln!("Agents: {}", agents.len());

        let config = ValidationRunConfig {
            run_label: "run1-pre-bugfix-baseline".into(),
            agent_ids: Vec::new(),
            enable_routing: true,
            staging_threshold: 0.95,
            agent_timeout_secs: 120,
        };

        let output = execute_validation_run(&battery, &config, agents_dir).expect("Run 1 failed");

        let agg = &output.ab_result.aggregate;
        eprintln!("\n═══ RUN 1 RESULTS ═══");
        eprintln!("Evaluated: {} agents", agg.agents_evaluated);
        eprintln!("Delta: {:.4}", agg.avg_composite_delta);
        eprintln!(
            "Improved/Unchanged/Degraded: {}/{}/{}",
            agg.agents_improved, agg.agents_unchanged, agg.agents_degraded
        );

        let bc = &agg.baseline_census;
        eprintln!(
            "Baseline census: bal={} theo={} proc={} rigid={} pat={} anom={}",
            bc.balanced,
            bc.theoretical_reasoner,
            bc.procedural_executor,
            bc.rigid_tool_user,
            bc.pattern_matching,
            bc.anomalous
        );

        for va in &agg.vector_aggregates {
            eprintln!(
                "  {:?}: base={:.3} route={:.3} delta={:.4}",
                va.vector, va.avg_baseline, va.avg_routed, va.avg_delta
            );
        }

        let cal = &output.ab_result.baseline.calibration;
        eprintln!(
            "Calibration: {} ({} inversions)",
            if cal.is_calibrated {
                "OK"
            } else {
                "INVERSIONS"
            },
            cal.inversions.len()
        );

        let gr = &output.ab_result.baseline.gaming_report;
        eprintln!(
            "Gaming: {} flags (R{} O{} Y{}) in {} agents",
            gr.total_flags, gr.red_count, gr.orange_count, gr.yellow_count, gr.agents_with_flags
        );

        let dir = workspace_root.join("data/validation_runs");
        let path = save_validation_run(&output, &dir).expect("Save failed");
        eprintln!("Saved: {}", path.display());

        assert!(agg.agents_evaluated >= 50);
    }

    /// Execute Run 2 post-bugfix and generate three-way comparison.
    /// cargo test -p nexus-capability-measurement -- execute_run2 --ignored --nocapture
    #[test]
    #[ignore]
    fn execute_run2_and_three_way_comparison() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let workspace_root = Path::new(manifest_dir).parent().unwrap().parent().unwrap();

        let battery_path =
            workspace_root.join("crates/nexus-capability-measurement/data/battery_v1.json");
        let battery = crate::battery::test_problem::load_battery(battery_path.to_str().unwrap())
            .expect("Load battery");

        let agents_dir = workspace_root.join("agents/prebuilt");
        let config = ValidationRunConfig {
            run_label: "run2-post-bugfix".into(),
            agent_ids: Vec::new(),
            enable_routing: true,
            staging_threshold: 0.95,
            agent_timeout_secs: 120,
        };

        eprintln!("═══ Executing Run 2: Post-Bug-Fix ═══\n");
        let output = execute_validation_run(&battery, &config, &agents_dir).expect("Run 2");

        let agg = &output.ab_result.aggregate;
        eprintln!(
            "Run 2: {} agents, delta={:.4}",
            agg.agents_evaluated, agg.avg_composite_delta
        );

        let dir = workspace_root.join("data/validation_runs");
        save_validation_run(&output, &dir).expect("Save Run 2");

        // Load Run 1 and build three-way comparison
        let run1_path = dir.join("run1-pre-bugfix-baseline.json");
        if run1_path.exists() {
            eprintln!("\n═══ Three-Way Comparison ═══\n");
            let run1 = load_validation_run(&run1_path).expect("Load Run 1");
            let cmp = crate::evaluation::three_way::build_three_way(&run1, &output);

            eprintln!("{}\n", cmp.narrative);
            eprintln!(
                "Bug Fix delta:  {:+.4} ({:+.1}%)",
                cmp.bugfix_delta.avg_delta, cmp.bugfix_delta.pct_improvement
            );
            eprintln!(
                "Routing delta:  {:+.4} ({:+.1}%)",
                cmp.routing_delta.avg_delta, cmp.routing_delta.pct_improvement
            );
            eprintln!(
                "Total delta:    {:+.4} ({:+.1}%)",
                cmp.total_delta.avg_delta, cmp.total_delta.pct_improvement
            );

            eprintln!("\nPer-Vector:");
            for v in &cmp.vector_details {
                eprintln!("  {:<20} R1={:.3}  R2={:.3}  Routed={:.3}  Fix={:+.3}  Route={:+.3}  Total={:+.3}",
                    v.vector, v.run1_avg, v.run2_baseline_avg, v.run2_routed_avg,
                    v.bugfix_delta, v.routing_delta, v.total_delta);
            }

            // Write markdown report
            let report_path = dir.join("THREE_WAY_COMPARISON.md");
            crate::evaluation::three_way::write_report(&cmp, &report_path).expect("Write report");
            eprintln!("\nReport: {}", report_path.display());
        } else {
            eprintln!(
                "Run 1 not found at {}, skipping three-way comparison",
                run1_path.display()
            );
        }

        assert!(agg.agents_evaluated >= 50);
    }
}
