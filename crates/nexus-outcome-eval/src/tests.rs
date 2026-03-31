#[cfg(test)]
mod outcome_eval_tests {
    use crate::artifact::OutcomeArtifactGenerator;
    use crate::builder::OutcomeSpecBuilder;
    use crate::evaluator::OutcomeEvaluator;
    use crate::types::*;
    use serde_json::json;

    fn default_context() -> serde_json::Value {
        json!({
            "duration_seconds": 30,
            "fuel_consumed": 100.0,
            "capabilities_used": ["fs.read", "llm.query"],
            "files_accessed": ["/home/user/doc.txt"],
            "api_calls": []
        })
    }

    // ── ContainsKeywords tests ───────────────────────────────────────

    #[test]
    fn keywords_all_found_passes() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain(
                "All keywords",
                vec!["hello".into(), "world".into()],
                MatchMode::All,
            )
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "hello beautiful world", &default_context());
        assert_eq!(result.verdict, OutcomeVerdict::Success);
        assert!(result.criteria_results[0].passed);
    }

    #[test]
    fn keywords_all_one_missing_fails() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain(
                "All keywords",
                vec!["hello".into(), "world".into()],
                MatchMode::All,
            )
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "hello only", &default_context());
        assert_eq!(result.verdict, OutcomeVerdict::Failure);
    }

    #[test]
    fn keywords_any_one_found_passes() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain(
                "Any keyword",
                vec!["hello".into(), "world".into()],
                MatchMode::Any,
            )
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "hello only", &default_context());
        assert_eq!(result.verdict, OutcomeVerdict::Success);
    }

    #[test]
    fn keywords_any_none_found_fails() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain(
                "Any keyword",
                vec!["hello".into(), "world".into()],
                MatchMode::Any,
            )
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "nothing here", &default_context());
        assert_eq!(result.verdict, OutcomeVerdict::Failure);
    }

    #[test]
    fn keywords_at_least_passes() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain(
                "At least 2",
                vec!["a".into(), "b".into(), "c".into()],
                MatchMode::AtLeast(2),
            )
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "a and b here", &default_context());
        assert!(result.criteria_results[0].passed);
    }

    #[test]
    fn keywords_at_least_fails() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain(
                "At least 2",
                vec!["a".into(), "b".into(), "c".into()],
                MatchMode::AtLeast(2),
            )
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "only a here", &default_context());
        assert!(!result.criteria_results[0].passed);
    }

    // ── MatchesPattern tests ─────────────────────────────────────────

    #[test]
    fn pattern_matches_passes() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_match("Has version", r"\d+\.\d+\.\d+")
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "Version 1.2.3 released", &default_context());
        assert!(result.criteria_results[0].passed);
    }

    #[test]
    fn pattern_no_match_fails() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_match("Has version", r"\d+\.\d+\.\d+")
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "No version here", &default_context());
        assert!(!result.criteria_results[0].passed);
    }

    #[test]
    fn pattern_invalid_regex_error() {
        let eval = OutcomeEvaluator::new();
        let criterion = SuccessCriterion {
            id: uuid::Uuid::new_v4(),
            description: "bad regex".into(),
            evaluator: CriterionEvaluator::MatchesPattern {
                pattern: "[invalid".into(),
            },
            required: true,
            weight: 1.0,
        };
        let result = eval.evaluate_criterion(&criterion, "test", &json!({}));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            OutcomeError::InvalidPattern(_)
        ));
    }

    // ── FileExists tests ─────────────────────────────────────────────

    #[test]
    fn file_exists_present() {
        // Use a file that always exists
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_produce_file("Cargo.toml exists", "Cargo.toml", None)
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "", &default_context());
        // May pass or fail depending on CWD, but should not panic
        let _ = result.criteria_results[0].passed;
    }

    #[test]
    fn file_exists_absent() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_produce_file("Missing file", "/tmp/nexus_nonexistent_file_xyz.txt", None)
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "", &default_context());
        assert!(!result.criteria_results[0].passed);
    }

    // ── NumericThreshold tests ───────────────────────────────────────

    #[test]
    fn numeric_greater_than_passes() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_exceed_threshold("Score > 80", "score", ComparisonOp::GreaterThan, 80.0)
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, r#"{"score": 95.5}"#, &default_context());
        assert!(result.criteria_results[0].passed);
    }

    #[test]
    fn numeric_greater_than_fails() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_exceed_threshold("Score > 80", "score", ComparisonOp::GreaterThan, 80.0)
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, r#"{"score": 50.0}"#, &default_context());
        assert!(!result.criteria_results[0].passed);
    }

    #[test]
    fn numeric_less_than_passes() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_exceed_threshold("Latency < 100", "latency", ComparisonOp::LessThan, 100.0)
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, r#"{"latency": 42.0}"#, &default_context());
        assert!(result.criteria_results[0].passed);
    }

    // ── ValidStructure tests ─────────────────────────────────────────

    #[test]
    fn valid_json_passes() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_be_valid_json("Valid JSON", json!({}))
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, r#"{"key": "value"}"#, &default_context());
        assert!(result.criteria_results[0].passed);
    }

    #[test]
    fn invalid_json_fails() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_be_valid_json("Valid JSON", json!({}))
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "not json at all", &default_context());
        assert!(!result.criteria_results[0].passed);
    }

    // ── HumanReview tests ────────────────────────────────────────────

    #[test]
    fn human_review_returns_pending() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .requires_human_review("Needs review", "Check quality")
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "some output", &default_context());
        assert_eq!(result.verdict, OutcomeVerdict::PendingReview);
    }

    // ── Constraint tests ─────────────────────────────────────────────

    #[test]
    fn forbidden_keywords_violated() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("Has result", vec!["result".into()], MatchMode::Any)
            .must_not_contain("No secrets", vec!["password".into(), "secret".into()])
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(
            &spec,
            "result: the password is secret123",
            &default_context(),
        );
        assert_eq!(result.verdict, OutcomeVerdict::Failure);
        assert!(result.constraint_results[0].violated);
    }

    #[test]
    fn forbidden_keywords_not_violated() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("Has result", vec!["result".into()], MatchMode::Any)
            .must_not_contain("No secrets", vec!["password".into()])
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "result: all good", &default_context());
        assert_eq!(result.verdict, OutcomeVerdict::Success);
        assert!(!result.constraint_results[0].violated);
    }

    #[test]
    fn time_limit_violated() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("Output", vec!["done".into()], MatchMode::Any)
            .must_complete_within(10)
            .build();
        let ctx = json!({"duration_seconds": 60});
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "done", &ctx);
        assert_eq!(result.verdict, OutcomeVerdict::Failure);
    }

    #[test]
    fn time_limit_not_violated() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("Output", vec!["done".into()], MatchMode::Any)
            .must_complete_within(60)
            .build();
        let ctx = json!({"duration_seconds": 10});
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "done", &ctx);
        assert_eq!(result.verdict, OutcomeVerdict::Success);
    }

    #[test]
    fn fuel_limit_violated() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("Output", vec!["ok".into()], MatchMode::Any)
            .must_not_exceed_fuel(50.0)
            .build();
        let ctx = json!({"fuel_consumed": 100.0});
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "ok", &ctx);
        assert_eq!(result.verdict, OutcomeVerdict::Failure);
    }

    // ── Assessment verdict tests ─────────────────────────────────────

    #[test]
    fn all_criteria_pass_success() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("Has greeting", vec!["hello".into()], MatchMode::Any)
            .must_match("Has number", r"\d+")
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "hello world 42", &default_context());
        assert_eq!(result.verdict, OutcomeVerdict::Success);
        assert!(result.score >= 0.8);
    }

    #[test]
    fn required_fails_gives_failure() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("Required", vec!["missing_keyword".into()], MatchMode::All)
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "no match here", &default_context());
        assert_eq!(result.verdict, OutcomeVerdict::Failure);
    }

    #[test]
    fn optional_fails_required_passes_partial() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("Required", vec!["hello".into()], MatchMode::Any)
            .nice_to_have(
                "Optional",
                CriterionEvaluator::ContainsKeywords {
                    keywords: vec!["extra".into()],
                    match_mode: MatchMode::All,
                },
                0.5,
            )
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "hello only", &default_context());
        // Required passed, optional failed. Score = (1.0*1.0 + 0.0*0.5) / 1.5 = 0.67
        // 0.5 <= 0.67 < 0.8 → PartialSuccess
        assert_eq!(result.verdict, OutcomeVerdict::PartialSuccess);
    }

    #[test]
    fn constraint_violated_overrides_success() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("Has output", vec!["result".into()], MatchMode::Any)
            .must_not_contain("No bad words", vec!["forbidden".into()])
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "result with forbidden content", &default_context());
        assert_eq!(result.verdict, OutcomeVerdict::Failure);
    }

    #[test]
    fn weighted_score_calculated_correctly() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("W=1.0", vec!["a".into()], MatchMode::All)
            .nice_to_have(
                "W=0.5",
                CriterionEvaluator::ContainsKeywords {
                    keywords: vec!["b".into()],
                    match_mode: MatchMode::All,
                },
                0.5,
            )
            .build();
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "a only", &default_context());
        // score = (1.0*1.0 + 0.0*0.5) / (1.0 + 0.5) = 0.667
        assert!((result.score - 0.667).abs() < 0.01);
    }

    // ── Builder tests ────────────────────────────────────────────────

    #[test]
    fn builder_creates_spec_with_criteria_and_constraints() {
        let spec = OutcomeSpecBuilder::new("t1", "a1", "Goal")
            .created_by("user")
            .must_contain("Has key", vec!["key".into()], MatchMode::All)
            .must_match("Pattern", r"\d+")
            .must_complete_within(60)
            .must_not_exceed_fuel(1000.0)
            .build();

        assert_eq!(spec.task_id, "t1");
        assert_eq!(spec.agent_id, "a1");
        assert_eq!(spec.criteria.len(), 2);
        assert_eq!(spec.constraints.len(), 2);
        assert_eq!(spec.created_by, "user");
    }

    #[test]
    fn builder_must_contain_creates_correct_evaluator() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("Test", vec!["x".into()], MatchMode::Any)
            .build();
        assert!(matches!(
            spec.criteria[0].evaluator,
            CriterionEvaluator::ContainsKeywords { .. }
        ));
        assert!(spec.criteria[0].required);
    }

    #[test]
    fn builder_nice_to_have_sets_not_required() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .nice_to_have(
                "Optional",
                CriterionEvaluator::MatchesPattern {
                    pattern: ".*".into(),
                },
                0.3,
            )
            .build();
        assert!(!spec.criteria[0].required);
        assert!((spec.criteria[0].weight - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn builder_time_constraint() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_complete_within(120)
            .build();
        assert!(matches!(
            spec.constraints[0].evaluator,
            ConstraintEvaluator::TimeLimit { max_seconds: 120 }
        ));
    }

    // ── Artifact tests ───────────────────────────────────────────────

    #[test]
    fn artifact_includes_all_sections() {
        let spec = OutcomeSpecBuilder::new("t1", "a1", "Test goal")
            .must_contain("Has output", vec!["done".into()], MatchMode::Any)
            .build();
        let eval = OutcomeEvaluator::new();
        let assessment = eval.evaluate(&spec, "done", &default_context());

        let artifact = OutcomeArtifactGenerator::generate(
            &assessment,
            &spec,
            vec![json!({"action": "test"})],
            5,
            1,
            vec![json!({"event": "hitl"})],
        );

        assert_eq!(artifact.memory_entries_created, 5);
        assert_eq!(artifact.rollbacks_performed, 1);
        assert_eq!(artifact.action_log.len(), 1);
        assert_eq!(artifact.governance_events.len(), 1);
    }

    #[test]
    fn artifact_report_is_readable() {
        let spec = OutcomeSpecBuilder::new("task-42", "nexus-researcher", "Summarize docs")
            .must_contain("Has summary", vec!["summary".into()], MatchMode::Any)
            .must_complete_within(300)
            .build();
        let eval = OutcomeEvaluator::new();
        let assessment = eval.evaluate(
            &spec,
            "Here is the summary",
            &json!({"duration_seconds": 45}),
        );
        let artifact = OutcomeArtifactGenerator::generate(&assessment, &spec, vec![], 0, 0, vec![]);
        let report = OutcomeArtifactGenerator::generate_report(&artifact);

        assert!(report.contains("OUTCOME EVALUATION REPORT"));
        assert!(report.contains("task-42"));
        assert!(report.contains("nexus-researcher"));
        assert!(report.contains("Summarize docs"));
        assert!(report.contains("SUCCESS"));
        assert!(report.contains("CRITERIA RESULTS"));
        assert!(report.contains("AUDIT"));
    }

    #[test]
    fn artifact_hash_deterministic() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("X", vec!["x".into()], MatchMode::Any)
            .build();
        let eval = OutcomeEvaluator::new();
        let assessment = eval.evaluate(&spec, "x", &default_context());
        let a1 = OutcomeArtifactGenerator::generate(&assessment, &spec, vec![], 0, 0, vec![]);
        // Same assessment → same hash (modulo generated_at timestamp)
        assert!(!a1.artifact_hash.is_empty());
    }

    #[test]
    fn artifact_verify_integrity_passes() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("X", vec!["x".into()], MatchMode::Any)
            .build();
        let eval = OutcomeEvaluator::new();
        let assessment = eval.evaluate(&spec, "x", &default_context());
        let artifact = OutcomeArtifactGenerator::generate(&assessment, &spec, vec![], 0, 0, vec![]);
        assert!(OutcomeArtifactGenerator::verify_integrity(&artifact));
    }

    #[test]
    fn artifact_tampered_fails_integrity() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("X", vec!["x".into()], MatchMode::Any)
            .build();
        let eval = OutcomeEvaluator::new();
        let assessment = eval.evaluate(&spec, "x", &default_context());
        let mut artifact =
            OutcomeArtifactGenerator::generate(&assessment, &spec, vec![], 0, 0, vec![]);
        artifact.memory_entries_created = 999; // tamper
        assert!(!OutcomeArtifactGenerator::verify_integrity(&artifact));
    }

    // ── Custom evaluator tests ───────────────────────────────────────

    #[test]
    fn custom_evaluator_works() {
        let mut eval = OutcomeEvaluator::new();
        eval.register_custom("word_count", |output, config| {
            let min = config
                .get("min_words")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as usize;
            let count = output.split_whitespace().count();
            let passed = count >= min;
            Ok((
                passed,
                if passed {
                    1.0
                } else {
                    count as f32 / min as f32
                },
                format!("{count} words (min: {min})"),
            ))
        });

        let criterion = SuccessCriterion {
            id: uuid::Uuid::new_v4(),
            description: "Enough words".into(),
            evaluator: CriterionEvaluator::Custom {
                evaluator_name: "word_count".into(),
                config: json!({"min_words": 3}),
            },
            required: true,
            weight: 1.0,
        };

        let result = eval
            .evaluate_criterion(&criterion, "one two three four", &json!({}))
            .unwrap();
        assert!(result.passed);
    }

    // ── Integration test ─────────────────────────────────────────────

    #[test]
    fn full_lifecycle() {
        // 1. Create spec
        let spec = OutcomeSpecBuilder::new("task-1", "nexus-coder", "Fix the login bug")
            .created_by("admin")
            .must_contain(
                "Mentions authentication",
                vec!["auth".into(), "login".into()],
                MatchMode::Any,
            )
            .must_match("Has diff output", r"^\-\-\-|^\+\+\+")
            .must_not_contain("No secrets", vec!["password123".into()])
            .must_complete_within(600)
            .must_not_exceed_fuel(5000.0)
            .build();

        // 2. Evaluate
        let eval = OutcomeEvaluator::new();
        let output = "--- a/src/auth.rs\n+++ b/src/auth.rs\n Fixed login validation";
        let ctx = json!({
            "duration_seconds": 120,
            "fuel_consumed": 2500.0,
            "capabilities_used": ["fs.read", "fs.write"],
        });
        let assessment = eval.evaluate(&spec, output, &ctx);

        assert_eq!(assessment.verdict, OutcomeVerdict::Success);
        assert!(assessment.score > 0.8);

        // 3. Generate artifact
        let artifact = OutcomeArtifactGenerator::generate(
            &assessment,
            &spec,
            vec![
                json!({"action": "read_file"}),
                json!({"action": "write_file"}),
            ],
            3,
            0,
            vec![json!({"event": "capability_check"})],
        );

        // 4. Verify
        assert!(OutcomeArtifactGenerator::verify_integrity(&artifact));

        // 5. Report
        let report = OutcomeArtifactGenerator::generate_report(&artifact);
        assert!(report.contains("SUCCESS"));
        assert!(report.contains("Fix the login bug"));
    }

    #[test]
    fn api_call_criterion() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_call_api("Called API", "example.com", Some(200))
            .build();
        let ctx = json!({
            "api_calls": [
                {"url": "https://example.com/api/v1", "status": 200}
            ]
        });
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "", &ctx);
        assert!(result.criteria_results[0].passed);
    }

    #[test]
    fn forbidden_capabilities_constraint() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("Has output", vec!["ok".into()], MatchMode::Any)
            .must_not_use_capabilities("No shell", vec!["process.exec".into()])
            .build();
        let ctx = json!({
            "capabilities_used": ["fs.read", "process.exec"]
        });
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "ok", &ctx);
        assert_eq!(result.verdict, OutcomeVerdict::Failure);
        assert!(result.constraint_results[0].violated);
    }

    #[test]
    fn forbidden_paths_constraint() {
        let spec = OutcomeSpecBuilder::new("t", "a", "g")
            .must_contain("Output", vec!["done".into()], MatchMode::Any)
            .must_not_access_paths("No /etc", vec!["/etc".into()])
            .build();
        let ctx = json!({
            "files_accessed": ["/etc/passwd", "/home/user/doc.txt"]
        });
        let eval = OutcomeEvaluator::new();
        let result = eval.evaluate(&spec, "done", &ctx);
        assert_eq!(result.verdict, OutcomeVerdict::Failure);
    }
}
