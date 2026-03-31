//! Outcome evaluator — assesses agent output against specifications.

use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

use crate::types::*;

type CustomEvalFn = Box<
    dyn Fn(&str, &serde_json::Value) -> Result<(bool, f32, String), OutcomeError> + Send + Sync,
>;

/// Evaluates agent outcomes against specifications.
pub struct OutcomeEvaluator {
    custom_evaluators: HashMap<String, CustomEvalFn>,
}

impl Default for OutcomeEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl OutcomeEvaluator {
    pub fn new() -> Self {
        Self {
            custom_evaluators: HashMap::new(),
        }
    }

    /// Register a custom evaluator function.
    pub fn register_custom(
        &mut self,
        name: &str,
        evaluator: impl Fn(&str, &serde_json::Value) -> Result<(bool, f32, String), OutcomeError>
            + Send
            + Sync
            + 'static,
    ) {
        self.custom_evaluators
            .insert(name.to_string(), Box::new(evaluator));
    }

    /// Evaluate a single criterion against agent output.
    pub fn evaluate_criterion(
        &self,
        criterion: &SuccessCriterion,
        agent_output: &str,
        task_context: &serde_json::Value,
    ) -> Result<CriterionResult, OutcomeError> {
        let (passed, score, evidence) = match &criterion.evaluator {
            CriterionEvaluator::ContainsKeywords {
                keywords,
                match_mode,
            } => eval_contains_keywords(agent_output, keywords, *match_mode),

            CriterionEvaluator::MatchesPattern { pattern } => {
                eval_matches_pattern(agent_output, pattern)?
            }

            CriterionEvaluator::LlmJudge { .. } => {
                return Err(OutcomeError::HumanReviewRequired(
                    "LLM judge not yet available; manual review required".into(),
                ));
            }

            CriterionEvaluator::FileExists {
                path,
                content_contains,
            } => eval_file_exists(path, content_contains.as_deref()),

            CriterionEvaluator::ApiCallMade {
                url_pattern,
                expected_status,
            } => eval_api_call_made(task_context, url_pattern, *expected_status),

            CriterionEvaluator::NumericThreshold {
                field,
                operator,
                threshold,
            } => eval_numeric_threshold(agent_output, field, *operator, *threshold)?,

            CriterionEvaluator::ValidStructure { schema } => {
                eval_valid_structure(agent_output, schema)
            }

            CriterionEvaluator::HumanReview {
                review_instructions,
            } => {
                return Err(OutcomeError::HumanReviewRequired(
                    review_instructions.clone(),
                ));
            }

            CriterionEvaluator::Custom {
                evaluator_name,
                config,
            } => {
                let eval_fn = self.custom_evaluators.get(evaluator_name).ok_or_else(|| {
                    OutcomeError::EvaluationFailed {
                        criterion_id: criterion.id,
                        reason: format!("custom evaluator '{evaluator_name}' not registered"),
                    }
                })?;
                eval_fn(agent_output, config)?
            }
        };

        Ok(CriterionResult {
            criterion_id: criterion.id,
            criterion_description: criterion.description.clone(),
            passed,
            score,
            evidence,
            evaluated_at: Utc::now(),
        })
    }

    /// Evaluate a single constraint.
    pub fn evaluate_constraint(
        &self,
        constraint: &Constraint,
        agent_output: &str,
        task_context: &serde_json::Value,
    ) -> Result<ConstraintResult, OutcomeError> {
        let (violated, evidence) = match &constraint.evaluator {
            ConstraintEvaluator::ForbiddenKeywords { keywords } => {
                eval_forbidden_keywords(agent_output, keywords)
            }
            ConstraintEvaluator::ForbiddenCapabilities { capabilities } => {
                eval_forbidden_capabilities(task_context, capabilities)
            }
            ConstraintEvaluator::TimeLimit { max_seconds } => {
                eval_time_limit(task_context, *max_seconds)
            }
            ConstraintEvaluator::FuelLimit { max_fuel } => eval_fuel_limit(task_context, *max_fuel),
            ConstraintEvaluator::ForbiddenPaths { paths } => {
                eval_forbidden_paths(task_context, paths)
            }
            ConstraintEvaluator::Custom {
                evaluator_name,
                config,
            } => {
                let eval_fn = self.custom_evaluators.get(evaluator_name).ok_or_else(|| {
                    OutcomeError::EvaluationFailed {
                        criterion_id: constraint.id,
                        reason: format!("custom evaluator '{evaluator_name}' not registered"),
                    }
                })?;
                let (passed, _score, evidence) = eval_fn(agent_output, config)?;
                (!passed, evidence)
            }
        };

        Ok(ConstraintResult {
            constraint_id: constraint.id,
            constraint_description: constraint.description.clone(),
            violated,
            evidence,
            evaluated_at: Utc::now(),
        })
    }

    /// Evaluate all criteria and constraints, producing an assessment.
    pub fn evaluate(
        &self,
        spec: &OutcomeSpec,
        agent_output: &str,
        task_context: &serde_json::Value,
    ) -> OutcomeAssessment {
        let start = Instant::now();
        let mut criteria_results = Vec::new();
        let mut has_pending_review = false;

        for criterion in &spec.criteria {
            match self.evaluate_criterion(criterion, agent_output, task_context) {
                Ok(result) => criteria_results.push(result),
                Err(OutcomeError::HumanReviewRequired(reason)) => {
                    has_pending_review = true;
                    criteria_results.push(CriterionResult {
                        criterion_id: criterion.id,
                        criterion_description: criterion.description.clone(),
                        passed: false,
                        score: 0.0,
                        evidence: format!("Pending human review: {reason}"),
                        evaluated_at: Utc::now(),
                    });
                }
                Err(e) => {
                    criteria_results.push(CriterionResult {
                        criterion_id: criterion.id,
                        criterion_description: criterion.description.clone(),
                        passed: false,
                        score: 0.0,
                        evidence: format!("Evaluation error: {e}"),
                        evaluated_at: Utc::now(),
                    });
                }
            }
        }

        let mut constraint_results = Vec::new();
        for constraint in &spec.constraints {
            match self.evaluate_constraint(constraint, agent_output, task_context) {
                Ok(result) => constraint_results.push(result),
                Err(e) => {
                    constraint_results.push(ConstraintResult {
                        constraint_id: constraint.id,
                        constraint_description: constraint.description.clone(),
                        violated: true,
                        evidence: format!("Evaluation error: {e}"),
                        evaluated_at: Utc::now(),
                    });
                }
            }
        }

        // Calculate weighted score
        let score = calculate_weighted_score(&spec.criteria, &criteria_results);

        // Determine verdict
        let any_required_failed =
            spec.criteria
                .iter()
                .zip(criteria_results.iter())
                .any(|(c, r)| {
                    c.required && !r.passed && !r.evidence.starts_with("Pending human review")
                });
        let any_constraint_violated = constraint_results.iter().any(|r| r.violated);

        let verdict = if any_constraint_violated {
            OutcomeVerdict::Failure
        } else if has_pending_review {
            OutcomeVerdict::PendingReview
        } else if any_required_failed {
            OutcomeVerdict::Failure
        } else if score >= 0.8 {
            OutcomeVerdict::Success
        } else if score >= 0.5 {
            OutcomeVerdict::PartialSuccess
        } else {
            OutcomeVerdict::Failure
        };

        let summary = build_summary(&verdict, score, &criteria_results, &constraint_results);
        let elapsed = start.elapsed().as_millis() as u64;

        let mut assessment = OutcomeAssessment {
            id: Uuid::new_v4(),
            spec_id: spec.id,
            task_id: spec.task_id.clone(),
            agent_id: spec.agent_id.clone(),
            agent_output: agent_output.to_string(),
            criteria_results,
            constraint_results,
            verdict,
            score,
            summary,
            evaluated_at: Utc::now(),
            evaluation_duration_ms: elapsed,
            audit_hash: String::new(),
        };

        assessment.audit_hash = compute_assessment_hash(&assessment);
        assessment
    }
}

// ── Criterion evaluators ────────────────────────────────────────────────

fn eval_contains_keywords(
    output: &str,
    keywords: &[String],
    mode: MatchMode,
) -> (bool, f32, String) {
    if keywords.is_empty() {
        return (true, 1.0, "No keywords to check".into());
    }
    let lower = output.to_lowercase();
    let found: Vec<&String> = keywords
        .iter()
        .filter(|kw| lower.contains(&kw.to_lowercase()))
        .collect();
    let found_count = found.len();
    let total = keywords.len();

    let passed = match mode {
        MatchMode::All => found_count == total,
        MatchMode::Any => found_count > 0,
        MatchMode::AtLeast(n) => found_count >= n,
    };

    // Score: 1.0 if passed, otherwise fractional
    let score = if passed {
        1.0
    } else {
        found_count as f32 / total as f32
    };

    let evidence = format!("Found {found_count}/{total} keywords");
    (passed, score, evidence)
}

fn eval_matches_pattern(output: &str, pattern: &str) -> Result<(bool, f32, String), OutcomeError> {
    let re = regex::Regex::new(pattern).map_err(|e| OutcomeError::InvalidPattern(e.to_string()))?;
    let matched = re.is_match(output);
    let score = if matched { 1.0 } else { 0.0 };
    let evidence = if matched {
        "Pattern matched".to_string()
    } else {
        format!("Pattern '{pattern}' did not match")
    };
    Ok((matched, score, evidence))
}

fn eval_file_exists(path: &str, content_contains: Option<&[String]>) -> (bool, f32, String) {
    if !std::path::Path::new(path).exists() {
        return (false, 0.0, format!("File not found: {path}"));
    }

    if let Some(required_content) = content_contains {
        if required_content.is_empty() {
            return (true, 1.0, format!("File exists: {path}"));
        }
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let found = required_content
                    .iter()
                    .filter(|c| content.contains(c.as_str()))
                    .count();
                let total = required_content.len();
                let score = found as f32 / total as f32;
                let passed = found == total;
                (
                    passed,
                    score,
                    format!("File exists, {found}/{total} content checks passed"),
                )
            }
            Err(e) => (false, 0.0, format!("File exists but unreadable: {e}")),
        }
    } else {
        (true, 1.0, format!("File exists: {path}"))
    }
}

fn eval_api_call_made(
    context: &serde_json::Value,
    url_pattern: &str,
    expected_status: Option<u16>,
) -> (bool, f32, String) {
    let calls = context
        .get("api_calls")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for call in &calls {
        let url = call.get("url").and_then(|v| v.as_str()).unwrap_or("");
        if url.contains(url_pattern) {
            if let Some(expected) = expected_status {
                let status = call.get("status").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
                if status == expected {
                    return (
                        true,
                        1.0,
                        format!("API call to '{url_pattern}' found with status {expected}"),
                    );
                }
            } else {
                return (true, 1.0, format!("API call to '{url_pattern}' found"));
            }
        }
    }
    (
        false,
        0.0,
        format!("No API call matching '{url_pattern}' found"),
    )
}

fn eval_numeric_threshold(
    output: &str,
    field: &str,
    op: ComparisonOp,
    threshold: f64,
) -> Result<(bool, f32, String), OutcomeError> {
    // Try parsing output as JSON and extracting field
    let value: f64 = if let Ok(json) = serde_json::from_str::<serde_json::Value>(output) {
        json.get(field)
            .and_then(|v| v.as_f64())
            .ok_or_else(|| OutcomeError::FieldNotFound(field.to_string()))?
    } else {
        // Try parsing the output itself as a number
        output
            .trim()
            .parse::<f64>()
            .map_err(|_| OutcomeError::FieldNotFound(field.to_string()))?
    };

    let passed = match op {
        ComparisonOp::GreaterThan => value > threshold,
        ComparisonOp::GreaterOrEqual => value >= threshold,
        ComparisonOp::LessThan => value < threshold,
        ComparisonOp::LessOrEqual => value <= threshold,
        ComparisonOp::Equal => (value - threshold).abs() < f64::EPSILON,
        ComparisonOp::NotEqual => (value - threshold).abs() >= f64::EPSILON,
    };

    let score = if passed { 1.0 } else { 0.0 };
    let evidence = format!("{field} = {value}, threshold {op:?} {threshold}: {passed}");
    Ok((passed, score, evidence))
}

fn eval_valid_structure(output: &str, _schema: &serde_json::Value) -> (bool, f32, String) {
    // Basic JSON validity check (full JSON Schema validation is Phase 2)
    match serde_json::from_str::<serde_json::Value>(output) {
        Ok(_) => (true, 1.0, "Valid JSON structure".into()),
        Err(e) => (false, 0.0, format!("Invalid JSON: {e}")),
    }
}

// ── Constraint evaluators ───────────────────────────────────────────────

fn eval_forbidden_keywords(output: &str, keywords: &[String]) -> (bool, String) {
    let lower = output.to_lowercase();
    let found: Vec<&String> = keywords
        .iter()
        .filter(|kw| lower.contains(&kw.to_lowercase()))
        .collect();
    if found.is_empty() {
        (false, "No forbidden keywords found".into())
    } else {
        (true, format!("Forbidden keywords found: {:?}", found))
    }
}

fn eval_forbidden_capabilities(
    context: &serde_json::Value,
    capabilities: &[String],
) -> (bool, String) {
    let used = context
        .get("capabilities_used")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let violations: Vec<&String> = capabilities
        .iter()
        .filter(|cap| {
            used.iter()
                .any(|u| u.as_str().map(|s| s == cap.as_str()).unwrap_or(false))
        })
        .collect();

    if violations.is_empty() {
        (false, "No forbidden capabilities used".into())
    } else {
        (
            true,
            format!("Forbidden capabilities used: {:?}", violations),
        )
    }
}

fn eval_time_limit(context: &serde_json::Value, max_seconds: u64) -> (bool, String) {
    let duration = context
        .get("duration_seconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if duration > max_seconds {
        (
            true,
            format!("Time limit exceeded: {duration}s > {max_seconds}s"),
        )
    } else {
        (
            false,
            format!("Within time limit: {duration}s <= {max_seconds}s"),
        )
    }
}

fn eval_fuel_limit(context: &serde_json::Value, max_fuel: f64) -> (bool, String) {
    let consumed = context
        .get("fuel_consumed")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    if consumed > max_fuel {
        (
            true,
            format!("Fuel limit exceeded: {consumed} > {max_fuel}"),
        )
    } else {
        (
            false,
            format!("Within fuel limit: {consumed} <= {max_fuel}"),
        )
    }
}

fn eval_forbidden_paths(context: &serde_json::Value, paths: &[String]) -> (bool, String) {
    let accessed = context
        .get("files_accessed")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let violations: Vec<String> = paths
        .iter()
        .filter(|p| {
            accessed.iter().any(|a| {
                a.as_str()
                    .map(|s| s.starts_with(p.as_str()))
                    .unwrap_or(false)
            })
        })
        .cloned()
        .collect();

    if violations.is_empty() {
        (false, "No forbidden paths accessed".into())
    } else {
        (true, format!("Forbidden paths accessed: {:?}", violations))
    }
}

// ── Scoring ─────────────────────────────────────────────────────────────

fn calculate_weighted_score(criteria: &[SuccessCriterion], results: &[CriterionResult]) -> f32 {
    if criteria.is_empty() || results.is_empty() {
        return 0.0;
    }
    let total_weight: f32 = criteria.iter().map(|c| c.weight).sum();
    if total_weight <= 0.0 {
        return 0.0;
    }
    let weighted_sum: f32 = criteria
        .iter()
        .zip(results.iter())
        .map(|(c, r)| r.score * c.weight)
        .sum();
    (weighted_sum / total_weight).clamp(0.0, 1.0)
}

fn build_summary(
    verdict: &OutcomeVerdict,
    score: f32,
    criteria_results: &[CriterionResult],
    constraint_results: &[ConstraintResult],
) -> String {
    let passed = criteria_results.iter().filter(|r| r.passed).count();
    let total = criteria_results.len();
    let violated = constraint_results.iter().filter(|r| r.violated).count();
    format!(
        "{verdict} (score: {score:.2}) — {passed}/{total} criteria passed, {violated} constraints violated"
    )
}

fn compute_assessment_hash(assessment: &OutcomeAssessment) -> String {
    let data = serde_json::json!({
        "id": assessment.id.to_string(),
        "spec_id": assessment.spec_id.to_string(),
        "task_id": assessment.task_id,
        "verdict": assessment.verdict,
        "score": assessment.score,
        "evaluated_at": assessment.evaluated_at.to_rfc3339(),
    });
    let serialized = serde_json::to_string(&data).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    hex::encode(hasher.finalize())
}
