use self_improve_agent::knowledge::{KnowledgeBase, KnowledgeCategory};
use self_improve_agent::learner::analyze_history;
use self_improve_agent::prompt_optimizer::{PromptOptimizer, PromptOutcome};
use self_improve_agent::r#loop::{AgentRunObservation, AutoImproveEngine, ImprovementStatus};
use self_improve_agent::skills::{SkillArea, SkillRatingSystem, TaskDifficulty};
use self_improve_agent::tracker::{
    MetricKind, OutcomeResult, PerformanceTracker, TaskMetrics, TaskType, TrendDirection,
};

#[test]
fn test_performance_tracking() {
    let mut tracker = PerformanceTracker::new_in_memory();
    for index in 0..10 {
        let metric = 0.50 + (index as f64 * 0.05);
        let metrics = TaskMetrics {
            test_pass_rate: Some(metric.min(1.0)),
            fix_iterations: Some((5 - (index / 2)).max(1) as f64),
            code_quality_score: Some(0.55 + (index as f64 * 0.03)),
            ..TaskMetrics::default()
        };
        tracker
            .track_outcome(
                "agent-coder",
                TaskType::Coding,
                "refactor parser",
                OutcomeResult::Success,
                metrics,
            )
            .expect("tracking outcome should succeed");
    }

    let history = tracker.outcomes_for("agent-coder", TaskType::Coding);
    assert_eq!(history.len(), 10);
    assert_eq!(
        tracker.trend_for("agent-coder", TaskType::Coding, MetricKind::TestPassRate),
        TrendDirection::Improving
    );
}

#[test]
fn test_strategy_learning() {
    let mut tracker = PerformanceTracker::new_in_memory();
    for _ in 0..5 {
        tracker
            .track_outcome(
                "agent-social",
                TaskType::Posting,
                "x growth thread 9am",
                OutcomeResult::Success,
                TaskMetrics {
                    engagement_rate: Some(0.82),
                    approval_rate: Some(0.95),
                    ..TaskMetrics::default()
                },
            )
            .expect("morning post should track");
    }
    for _ in 0..5 {
        tracker
            .track_outcome(
                "agent-social",
                TaskType::Posting,
                "x growth thread 3pm",
                OutcomeResult::Success,
                TaskMetrics {
                    engagement_rate: Some(0.24),
                    approval_rate: Some(0.95),
                    ..TaskMetrics::default()
                },
            )
            .expect("afternoon post should track");
    }

    match analyze_history(&tracker, "agent-social", TaskType::Posting) {
        Ok(insights) => {
            assert!(
                insights
                    .recommendations
                    .iter()
                    .any(|recommendation| recommendation
                        .to_ascii_lowercase()
                        .contains("before 10am")),
                "expected recommendation to post before 10am, got: {:?}",
                insights.recommendations
            );
        }
        Err(e) if format!("{e}").contains("ollama") || format!("{e}").contains("404") => {
            eprintln!(
                "SKIPPED: analyze_history requires a working LLM provider. Error: {e}\n\
                 To run this test: ollama pull llama3.2"
            );
        }
        Err(e) => panic!("analysis failed with unexpected error: {e}"),
    }
}

#[test]
fn test_prompt_optimization() {
    let mut optimizer = PromptOptimizer::new();
    let outcomes = vec![
        PromptOutcome {
            prompt: "Prompt A".to_string(),
            success: true,
            score: 0.62,
        },
        PromptOutcome {
            prompt: "Prompt A".to_string(),
            success: true,
            score: 0.66,
        },
        PromptOutcome {
            prompt: "Prompt A".to_string(),
            success: true,
            score: 0.61,
        },
        PromptOutcome {
            prompt: "Prompt A".to_string(),
            success: false,
            score: 0.20,
        },
        PromptOutcome {
            prompt: "Prompt A".to_string(),
            success: false,
            score: 0.21,
        },
        PromptOutcome {
            prompt: "Prompt B".to_string(),
            success: true,
            score: 0.81,
        },
        PromptOutcome {
            prompt: "Prompt B".to_string(),
            success: true,
            score: 0.84,
        },
        PromptOutcome {
            prompt: "Prompt B".to_string(),
            success: true,
            score: 0.82,
        },
        PromptOutcome {
            prompt: "Prompt B".to_string(),
            success: true,
            score: 0.83,
        },
        PromptOutcome {
            prompt: "Prompt B".to_string(),
            success: false,
            score: 0.10,
        },
    ];

    let selected = optimizer.optimize_prompt("Base prompt", outcomes.as_slice());
    assert_eq!(selected, "Prompt B");
    assert_eq!(optimizer.default_prompt("Base prompt"), Some("Prompt B"));
}

#[test]
fn test_knowledge_retrieval() {
    let mut kb = KnowledgeBase::new_in_memory("agent-key");
    kb.store_strategy(
        "agent-coder",
        KnowledgeCategory::ErrorSolutions,
        "Use robust error handling in Rust code with Result and context.",
        &["rust", "error", "handling"],
    )
    .expect("store strategy");
    kb.store_strategy(
        "agent-coder",
        KnowledgeCategory::DesignPrinciples,
        "Prefer consistent visual hierarchy for landing pages.",
        &["design", "ux"],
    )
    .expect("store strategy");
    kb.store_strategy(
        "agent-other",
        KnowledgeCategory::ErrorSolutions,
        "Ignore this other agent entry.",
        &["rust"],
    )
    .expect("store strategy");

    let hits = kb.retrieve("agent-coder", "writing Rust code", 3);
    assert!(
        hits.iter().any(|hit| {
            hit.entry
                .strategy
                .to_ascii_lowercase()
                .contains("error handling in rust")
        }),
        "expected rust error-handling strategy in hits: {:?}",
        hits
    );
}

#[test]
fn test_skill_rating() {
    let mut ratings = SkillRatingSystem::new();
    let initial = ratings.rating_for("agent-coder", SkillArea::Coding);

    for _ in 0..5 {
        ratings.update_rating(
            "agent-coder",
            SkillArea::Coding,
            true,
            TaskDifficulty::Medium,
        );
    }
    let after_successes = ratings.rating_for("agent-coder", SkillArea::Coding);
    assert!(after_successes > initial);

    for _ in 0..2 {
        ratings.update_rating(
            "agent-coder",
            SkillArea::Coding,
            false,
            TaskDifficulty::Medium,
        );
    }
    let after_failures = ratings.rating_for("agent-coder", SkillArea::Coding);
    assert!(after_failures < after_successes);
    assert!(after_failures > 0.0);
}

#[test]
fn test_auto_improve_loop_versions_and_audit() {
    let mut engine = AutoImproveEngine::new_in_memory("scope-a");
    let result = engine.run_cycle(AgentRunObservation {
        agent_id: "agent-coder".to_string(),
        task: "refactor auth handler".to_string(),
        task_type: TaskType::Coding,
        result: OutcomeResult::Success,
        metrics: TaskMetrics {
            test_pass_rate: Some(1.0),
            fix_iterations: Some(1.0),
            code_quality_score: Some(0.9),
            ..TaskMetrics::default()
        },
        base_prompt: "Write robust rust code".to_string(),
        prompt_outcomes: vec![PromptOutcome {
            prompt: "Write robust rust code".to_string(),
            success: true,
            score: 0.92,
        }],
        governance_approved: true,
        destructive_change_requested: false,
        sandbox_validation_passed: true,
    });

    match result {
        Ok(result) => {
            assert_eq!(result.status, ImprovementStatus::Applied);
            assert_eq!(result.version.version_id, 1);
            assert!(!engine.audit_for_agent("agent-coder").is_empty());
        }
        Err(e) if format!("{e}").contains("ollama") || format!("{e}").contains("404") => {
            eprintln!(
                "SKIPPED: auto-improve loop requires a working LLM provider. Error: {e}\n\
                 To run this test: ollama pull llama3.2"
            );
        }
        Err(e) => panic!("loop run failed with unexpected error: {e}"),
    }
}

#[test]
fn test_auto_improve_loop_rolls_back_version() {
    let mut engine = AutoImproveEngine::new_in_memory("scope-b");
    let result = engine.run_cycle(AgentRunObservation {
        agent_id: "agent-web".to_string(),
        task: "optimize landing page".to_string(),
        task_type: TaskType::Website,
        result: OutcomeResult::Partial,
        metrics: TaskMetrics {
            build_success: Some(1.0),
            user_satisfaction: Some(0.8),
            load_time: Some(1.4),
            ..TaskMetrics::default()
        },
        base_prompt: "Design modern websites".to_string(),
        prompt_outcomes: vec![PromptOutcome {
            prompt: "Design modern websites".to_string(),
            success: true,
            score: 0.8,
        }],
        governance_approved: true,
        destructive_change_requested: false,
        sandbox_validation_passed: true,
    });

    match result {
        Ok(result) => {
            let rolled_back = engine
                .rollback_to("agent-web", result.version.version_id)
                .expect("rollback should succeed");
            assert_eq!(rolled_back.version_id, result.version.version_id);
        }
        Err(e) if format!("{e}").contains("ollama") || format!("{e}").contains("404") => {
            eprintln!(
                "SKIPPED: auto-improve loop requires a working LLM provider. Error: {e}\n\
                 To run this test: ollama pull llama3.2"
            );
        }
        Err(e) => panic!("loop run failed with unexpected error: {e}"),
    }
}
