use self_improve_agent::knowledge::{KnowledgeBase, KnowledgeCategory};
use self_improve_agent::learner::analyze_history;
use self_improve_agent::prompt_optimizer::{PromptOptimizer, PromptOutcome};
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

    let insights =
        analyze_history(&tracker, "agent-social", TaskType::Posting).expect("analysis succeeds");
    assert!(
        insights
            .recommendations
            .iter()
            .any(|recommendation| recommendation.to_ascii_lowercase().contains("before 10am")),
        "expected recommendation to post before 10am, got: {:?}",
        insights.recommendations
    );
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
