//! Self-improvement engine for tracking outcomes, learning strategies, optimizing prompts,
//! managing per-agent knowledge, and rating skills over time.

pub mod knowledge;
pub mod learner;
pub mod r#loop;
pub mod prompt_optimizer;
pub mod skills;
pub mod tracker;

#[cfg(test)]
mod tests {
    use super::*;

    // ── Skill rating system (Elo) ──

    #[test]
    fn default_rating_is_1200() {
        let sys = skills::SkillRatingSystem::new();
        assert_eq!(sys.rating_for("agent-1", skills::SkillArea::Coding), 1200.0);
    }

    #[test]
    fn success_increases_rating() {
        let mut sys = skills::SkillRatingSystem::new();
        let before = sys.rating_for("a", skills::SkillArea::Coding);
        sys.update_rating(
            "a",
            skills::SkillArea::Coding,
            true,
            skills::TaskDifficulty::Medium,
        );
        let after = sys.rating_for("a", skills::SkillArea::Coding);
        assert!(
            after > before,
            "success should increase rating: {before} -> {after}"
        );
    }

    #[test]
    fn failure_decreases_rating() {
        let mut sys = skills::SkillRatingSystem::new();
        let before = sys.rating_for("a", skills::SkillArea::Coding);
        sys.update_rating(
            "a",
            skills::SkillArea::Coding,
            false,
            skills::TaskDifficulty::Medium,
        );
        let after = sys.rating_for("a", skills::SkillArea::Coding);
        assert!(
            after < before,
            "failure should decrease rating: {before} -> {after}"
        );
    }

    #[test]
    fn rating_never_below_one() {
        let mut sys = skills::SkillRatingSystem::new();
        for _ in 0..200 {
            sys.update_rating(
                "weak",
                skills::SkillArea::Coding,
                false,
                skills::TaskDifficulty::Expert,
            );
        }
        assert!(sys.rating_for("weak", skills::SkillArea::Coding) >= 1.0);
    }

    #[test]
    fn level_thresholds() {
        let mut sys = skills::SkillRatingSystem::new();
        // Default 1200 = Intermediate
        assert_eq!(
            sys.level_for("a", skills::SkillArea::Coding),
            skills::SkillLevel::Intermediate
        );
        // Push high
        for _ in 0..100 {
            sys.update_rating(
                "a",
                skills::SkillArea::Coding,
                true,
                skills::TaskDifficulty::Expert,
            );
        }
        assert_eq!(
            sys.level_for("a", skills::SkillArea::Coding),
            skills::SkillLevel::Expert
        );
    }

    #[test]
    fn leaderboard_sorted_descending() {
        let mut sys = skills::SkillRatingSystem::new();
        sys.update_rating(
            "alice",
            skills::SkillArea::Writing,
            true,
            skills::TaskDifficulty::Hard,
        );
        sys.update_rating(
            "bob",
            skills::SkillArea::Writing,
            false,
            skills::TaskDifficulty::Easy,
        );
        let board = sys.leaderboard(skills::SkillArea::Writing);
        assert!(board.len() >= 2);
        assert!(board[0].rating >= board[1].rating);
    }

    #[test]
    fn recommended_difficulty_scales_with_rating() {
        let mut sys = skills::SkillRatingSystem::new();
        // Default 1200 → Medium
        assert_eq!(
            sys.recommended_difficulty("a", skills::SkillArea::Coding),
            skills::TaskDifficulty::Medium
        );
        for _ in 0..60 {
            sys.update_rating(
                "a",
                skills::SkillArea::Coding,
                true,
                skills::TaskDifficulty::Expert,
            );
        }
        assert_eq!(
            sys.recommended_difficulty("a", skills::SkillArea::Coding),
            skills::TaskDifficulty::Expert
        );
    }

    // ── Prompt optimizer ──

    #[test]
    fn optimize_selects_highest_success_rate() {
        let mut opt = prompt_optimizer::PromptOptimizer::new();
        let outcomes = vec![
            prompt_optimizer::PromptOutcome {
                prompt: "bad prompt".into(),
                success: false,
                score: 0.1,
            },
            prompt_optimizer::PromptOutcome {
                prompt: "good prompt".into(),
                success: true,
                score: 0.9,
            },
            prompt_optimizer::PromptOutcome {
                prompt: "good prompt".into(),
                success: true,
                score: 0.8,
            },
        ];
        let best = opt.optimize_prompt("base", &outcomes);
        assert_eq!(best, "good prompt");
    }

    #[test]
    fn default_prompt_set_and_retrieved() {
        let mut opt = prompt_optimizer::PromptOptimizer::new();
        assert!(opt.default_prompt("key").is_none());
        opt.set_default_prompt("key", "custom");
        assert_eq!(opt.default_prompt("key"), Some("custom"));
    }

    #[test]
    fn variants_tracked_correctly() {
        let mut opt = prompt_optimizer::PromptOptimizer::new();
        let outcomes = vec![
            prompt_optimizer::PromptOutcome {
                prompt: "v1".into(),
                success: true,
                score: 0.9,
            },
            prompt_optimizer::PromptOutcome {
                prompt: "v2".into(),
                success: false,
                score: 0.3,
            },
        ];
        opt.optimize_prompt("base", &outcomes);
        let variants = opt.variants_for("base");
        assert!(variants.len() >= 3); // base + v1 + v2
    }

    #[test]
    fn context_hints_by_task_type() {
        let coding_hints = prompt_optimizer::PromptOptimizer::context_hints("Coding");
        assert!(!coding_hints.is_empty());
        assert!(coding_hints.iter().any(|h| h.contains("test")));
        let other_hints = prompt_optimizer::PromptOptimizer::context_hints("unknown");
        assert!(!other_hints.is_empty());
    }

    // ── Performance tracker (in-memory) ──

    #[test]
    fn track_outcome_assigns_incrementing_ids() {
        let mut t = tracker::PerformanceTracker::new_in_memory();
        let o1 = t
            .track_outcome(
                "a",
                tracker::TaskType::Coding,
                "task1",
                tracker::OutcomeResult::Success,
                tracker::TaskMetrics::default(),
            )
            .unwrap();
        let o2 = t
            .track_outcome(
                "a",
                tracker::TaskType::Coding,
                "task2",
                tracker::OutcomeResult::Failure,
                tracker::TaskMetrics::default(),
            )
            .unwrap();
        assert!(o2.id > o1.id);
    }

    #[test]
    fn outcomes_filtered_by_agent_and_type() {
        let mut t = tracker::PerformanceTracker::new_in_memory();
        t.track_outcome(
            "a",
            tracker::TaskType::Coding,
            "c1",
            tracker::OutcomeResult::Success,
            tracker::TaskMetrics::default(),
        )
        .unwrap();
        t.track_outcome(
            "a",
            tracker::TaskType::Posting,
            "p1",
            tracker::OutcomeResult::Success,
            tracker::TaskMetrics::default(),
        )
        .unwrap();
        t.track_outcome(
            "b",
            tracker::TaskType::Coding,
            "c2",
            tracker::OutcomeResult::Success,
            tracker::TaskMetrics::default(),
        )
        .unwrap();
        let coding_a = t.outcomes_for("a", tracker::TaskType::Coding);
        assert_eq!(coding_a.len(), 1);
        assert_eq!(coding_a[0].task, "c1");
    }

    #[test]
    fn trend_insufficient_data_for_few_outcomes() {
        let t = tracker::PerformanceTracker::new_in_memory();
        assert_eq!(
            t.trend_for(
                "a",
                tracker::TaskType::Coding,
                tracker::MetricKind::TestPassRate
            ),
            tracker::TrendDirection::InsufficientData
        );
    }

    // ── Knowledge base (in-memory) ──

    #[test]
    fn store_and_retrieve_strategy() {
        let mut kb = knowledge::KnowledgeBase::new_in_memory("test-scope");
        kb.store_strategy(
            "a",
            knowledge::KnowledgeCategory::CodingPatterns,
            "use iterators over for loops",
            &["rust", "perf"],
        )
        .unwrap();
        let hits = kb.retrieve("a", "iterators performance", 5);
        assert!(!hits.is_empty());
        assert!(hits[0].similarity > 0.0);
    }

    #[test]
    fn retrieve_returns_empty_for_no_match() {
        let kb = knowledge::KnowledgeBase::new_in_memory("scope");
        let hits = kb.retrieve("a", "completely unrelated query", 5);
        assert!(hits.is_empty());
    }

    #[test]
    fn knowledge_entries_accumulate() {
        let mut kb = knowledge::KnowledgeBase::new_in_memory("scope");
        kb.store_strategy("a", knowledge::KnowledgeCategory::ErrorFixes, "fix1", &[])
            .unwrap();
        kb.store_strategy("a", knowledge::KnowledgeCategory::ErrorFixes, "fix2", &[])
            .unwrap();
        assert_eq!(kb.entries().len(), 2);
    }
}
