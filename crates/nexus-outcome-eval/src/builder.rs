//! Fluent builder for outcome specifications.

use chrono::Utc;
use uuid::Uuid;

use crate::types::*;

/// Fluent builder for [`OutcomeSpec`].
pub struct OutcomeSpecBuilder {
    task_id: String,
    agent_id: String,
    goal: String,
    criteria: Vec<SuccessCriterion>,
    constraints: Vec<Constraint>,
    created_by: String,
}

impl OutcomeSpecBuilder {
    pub fn new(task_id: &str, agent_id: &str, goal: &str) -> Self {
        Self {
            task_id: task_id.to_string(),
            agent_id: agent_id.to_string(),
            goal: goal.to_string(),
            criteria: Vec::new(),
            constraints: Vec::new(),
            created_by: "system".to_string(),
        }
    }

    pub fn created_by(mut self, by: &str) -> Self {
        self.created_by = by.to_string();
        self
    }

    // ── Criterion builders ───────────────────────────────────────────

    fn add_criterion(
        &mut self,
        description: &str,
        evaluator: CriterionEvaluator,
        required: bool,
        weight: f32,
    ) {
        self.criteria.push(SuccessCriterion {
            id: Uuid::new_v4(),
            description: description.to_string(),
            evaluator,
            required,
            weight,
        });
    }

    pub fn must_contain(
        mut self,
        description: &str,
        keywords: Vec<String>,
        mode: MatchMode,
    ) -> Self {
        self.add_criterion(
            description,
            CriterionEvaluator::ContainsKeywords {
                keywords,
                match_mode: mode,
            },
            true,
            1.0,
        );
        self
    }

    pub fn must_match(mut self, description: &str, pattern: &str) -> Self {
        self.add_criterion(
            description,
            CriterionEvaluator::MatchesPattern {
                pattern: pattern.to_string(),
            },
            true,
            1.0,
        );
        self
    }

    pub fn must_produce_file(
        mut self,
        description: &str,
        path: &str,
        content_contains: Option<Vec<String>>,
    ) -> Self {
        self.add_criterion(
            description,
            CriterionEvaluator::FileExists {
                path: path.to_string(),
                content_contains,
            },
            true,
            1.0,
        );
        self
    }

    pub fn must_call_api(
        mut self,
        description: &str,
        url_pattern: &str,
        expected_status: Option<u16>,
    ) -> Self {
        self.add_criterion(
            description,
            CriterionEvaluator::ApiCallMade {
                url_pattern: url_pattern.to_string(),
                expected_status,
            },
            true,
            1.0,
        );
        self
    }

    pub fn must_exceed_threshold(
        mut self,
        description: &str,
        field: &str,
        op: ComparisonOp,
        threshold: f64,
    ) -> Self {
        self.add_criterion(
            description,
            CriterionEvaluator::NumericThreshold {
                field: field.to_string(),
                operator: op,
                threshold,
            },
            true,
            1.0,
        );
        self
    }

    pub fn must_be_valid_json(mut self, description: &str, schema: serde_json::Value) -> Self {
        self.add_criterion(
            description,
            CriterionEvaluator::ValidStructure { schema },
            true,
            1.0,
        );
        self
    }

    pub fn requires_human_review(mut self, description: &str, instructions: &str) -> Self {
        self.add_criterion(
            description,
            CriterionEvaluator::HumanReview {
                review_instructions: instructions.to_string(),
            },
            true,
            1.0,
        );
        self
    }

    pub fn nice_to_have(
        mut self,
        description: &str,
        evaluator: CriterionEvaluator,
        weight: f32,
    ) -> Self {
        self.add_criterion(description, evaluator, false, weight);
        self
    }

    // ── Constraint builders ──────────────────────────────────────────

    fn add_constraint(&mut self, description: &str, evaluator: ConstraintEvaluator) {
        self.constraints.push(Constraint {
            id: Uuid::new_v4(),
            description: description.to_string(),
            evaluator,
        });
    }

    pub fn must_not_contain(mut self, description: &str, forbidden: Vec<String>) -> Self {
        self.add_constraint(
            description,
            ConstraintEvaluator::ForbiddenKeywords {
                keywords: forbidden,
            },
        );
        self
    }

    pub fn must_not_use_capabilities(mut self, description: &str, caps: Vec<String>) -> Self {
        self.add_constraint(
            description,
            ConstraintEvaluator::ForbiddenCapabilities { capabilities: caps },
        );
        self
    }

    pub fn must_complete_within(mut self, seconds: u64) -> Self {
        self.add_constraint(
            &format!("Must complete within {seconds}s"),
            ConstraintEvaluator::TimeLimit {
                max_seconds: seconds,
            },
        );
        self
    }

    pub fn must_not_exceed_fuel(mut self, max_fuel: f64) -> Self {
        self.add_constraint(
            &format!("Fuel must not exceed {max_fuel}"),
            ConstraintEvaluator::FuelLimit { max_fuel },
        );
        self
    }

    pub fn must_not_access_paths(mut self, description: &str, paths: Vec<String>) -> Self {
        self.add_constraint(description, ConstraintEvaluator::ForbiddenPaths { paths });
        self
    }

    // ── Build ────────────────────────────────────────────────────────

    pub fn build(self) -> OutcomeSpec {
        OutcomeSpec {
            id: Uuid::new_v4(),
            task_id: self.task_id,
            agent_id: self.agent_id,
            goal_description: self.goal,
            criteria: self.criteria,
            constraints: self.constraints,
            created_by: self.created_by,
            created_at: Utc::now(),
        }
    }
}
