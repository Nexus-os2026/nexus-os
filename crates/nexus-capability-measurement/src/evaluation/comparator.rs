//! Agent response vs expected chain comparison.
//!
//! Two scoring paths:
//! - **Keyword-based** (`compare_response`): fast, no LLM needed, used as fallback
//! - **LLM-as-judge** (`ResponseComparator`): semantic comparison via external LLM

use serde::{Deserialize, Serialize};

use crate::battery::expected_chain::ExpectedReasoning;
use crate::battery::test_problem::TestProblem;
use crate::framework::{DifficultyLevel, Vector};
use crate::scoring::articulation::{ArticulationDimension, ArticulationScore};
use crate::scoring::asymmetric::{apply_penalties, Penalty, PenaltySeverity, PrimaryScore};
use crate::scoring::gaming_detection::{GamingFlag, GamingFlagSeverity, GamingFlagType};

// ── Keyword-Based Comparison (Fallback) ──────────────────────────────────────

/// Compare agent response text against the expected reasoning chain.
/// Returns (coverage, gap_count, redundancy_count, hallucination_count).
pub fn compare_response(
    response: &str,
    expected: &ExpectedReasoning,
) -> (f64, usize, usize, usize) {
    let resp_lower = response.to_lowercase();

    let insights_found = expected
        .required_insights
        .iter()
        .filter(|insight| {
            let keywords: Vec<&str> = insight.split_whitespace().collect();
            let matched = keywords
                .iter()
                .filter(|kw| resp_lower.contains(&kw.to_lowercase()))
                .count();
            matched * 2 >= keywords.len()
        })
        .count();

    let total_insights = expected.required_insights.len().max(1);
    let coverage = insights_found as f64 / total_insights as f64;
    let gap_count = total_insights - insights_found;

    let sentences: Vec<&str> = response
        .split('.')
        .map(|s| s.trim())
        .filter(|s| s.len() > 10)
        .collect();
    let unique_sentences: std::collections::HashSet<&str> = sentences.iter().copied().collect();
    let redundancy_count = sentences.len().saturating_sub(unique_sentences.len());

    let hallucination_count = expected
        .critical_failures
        .iter()
        .filter(|cf| {
            let keywords: Vec<&str> = cf.split_whitespace().collect();
            let matched = keywords
                .iter()
                .filter(|kw| resp_lower.contains(&kw.to_lowercase()))
                .count();
            matched * 2 >= keywords.len()
        })
        .count();

    (coverage, gap_count, redundancy_count, hallucination_count)
}

// ── LLM-as-Judge Comparator ──────────────────────────────────────────────────

/// Type alias for the judge function to keep the struct simple.
type JudgeFn = dyn Fn(&str) -> Result<String, String> + Send + Sync;

/// Compares agent responses against expected reasoning chains using LLM-as-judge.
pub struct ResponseComparator {
    /// Closure that sends a prompt to a judge LLM and returns the response text.
    judge: Box<JudgeFn>,
}

impl ResponseComparator {
    /// Create a new comparator with the given judge function.
    pub fn new(judge: impl Fn(&str) -> Result<String, String> + Send + Sync + 'static) -> Self {
        Self {
            judge: Box::new(judge),
        }
    }

    /// Score the primary reasoning quality using LLM-as-judge.
    pub fn score_primary(
        &self,
        problem: &TestProblem,
        response: &str,
        vector: Vector,
    ) -> Result<PrimaryScore, String> {
        let prompt = build_primary_judge_prompt(problem, response, vector);
        let judge_response = (self.judge)(&prompt)?;
        parse_primary_score(&judge_response, vector)
    }

    /// Score articulation quality using LLM-as-judge.
    pub fn score_articulation(
        &self,
        problem: &TestProblem,
        response: &str,
        vector: Vector,
    ) -> Result<ArticulationScore, String> {
        let prompt = build_articulation_judge_prompt(problem, response, vector);
        let judge_response = (self.judge)(&prompt)?;
        parse_articulation_score(&judge_response, vector)
    }

    /// Detect gaming using LLM-as-judge.
    pub fn detect_gaming(
        &self,
        problem: &TestProblem,
        response: &str,
        vector: Vector,
    ) -> Result<Vec<GamingFlag>, String> {
        let prompt = build_gaming_judge_prompt(problem, response, vector);
        let judge_response = (self.judge)(&prompt)?;
        parse_gaming_flags(&judge_response)
    }
}

// ── Single-Problem Evaluation Result ─────────────────────────────────────────

/// Result of evaluating a single agent response against one problem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleEvaluationResult {
    pub problem_id: String,
    pub vector: Vector,
    pub level: DifficultyLevel,
    pub primary_score: PrimaryScore,
    pub articulation_score: ArticulationScore,
    pub gaming_flags: Vec<GamingFlag>,
}

// ── Prompt Builders ──────────────────────────────────────────────────────────

/// Build the judge prompt for primary scoring.
pub fn build_primary_judge_prompt(problem: &TestProblem, response: &str, vector: Vector) -> String {
    let expected_chain = format_expected_chain(&problem.expected_reasoning, vector);
    let rubric = &problem.scoring_rubric;
    let critical = problem.expected_reasoning.critical_failures.join("; ");
    let vector_name = format!("{vector:?}");
    let level = format!("{:?}", problem.level);

    format!(
        r#"You are a capability measurement judge. Compare an AI agent's response against an expert-documented expected reasoning chain and produce a structured score.

PROBLEM:
{problem_statement}

EXPECTED REASONING CHAIN:
{expected_chain}

AGENT'S RESPONSE:
{response}

SCORING RUBRIC:
- Full credit criteria: {full_credit}
- Partial credit criteria: {partial_credit}
- Zero credit criteria: {zero_credit}
- Critical failures (automatic zero): {critical}

VECTOR: {vector_name}
DIFFICULTY: {level}

Score the agent's response. Return ONLY a JSON object:
{{"raw_score": <float 0.0-1.0>, "penalties": [{{"reason": "<reason>", "severity": "<Minor|Major|Critical|Catastrophic>", "weight": <float>}}], "reasoning": "<1-2 sentences>", "critical_failure_triggered": <true|false>, "critical_failure_reason": "<reason or null>"}}

SCORING RULES:
- 1.0 = full credit: all required insights identified, all chains traced, no fabrication
- 0.7-0.9 = partial: most but not all chains identified
- 0.4-0.6 = weak: addressed the problem but missed key relationships
- 0.1-0.3 = poor: superficial or pattern matching
- 0.0 = critical failure triggered

Return ONLY the JSON. No markdown, no explanation outside the JSON."#,
        problem_statement = problem.problem_statement,
        full_credit = rubric.full_credit.join("; "),
        partial_credit = rubric.partial_credit.join("; "),
        zero_credit = rubric.zero_credit.join("; "),
    )
}

/// Build the judge prompt for articulation scoring.
pub fn build_articulation_judge_prompt(
    _problem: &TestProblem,
    response: &str,
    vector: Vector,
) -> String {
    let dims = crate::scoring::articulation::articulation_dimensions(vector);
    let dim_defs = articulation_dimension_definitions(vector);
    let vector_name = format!("{vector:?}");

    let dims_json: String = dims
        .iter()
        .map(|d| format!(r#"{{"name": "{d}", "score": 0, "evidence": ""}}"#))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        r#"You are scoring an AI agent's ability to ARTICULATE its reasoning, not just produce correct answers.

AGENT'S RESPONSE:
{response}

VECTOR: {vector_name}

Score each dimension as 0 (absent) or 1 (present). Return ONLY a JSON object:
{{"dimensions": [{dims_json}]}}

DIMENSION DEFINITIONS:
{dim_defs}

Return ONLY the JSON."#
    )
}

/// Build the judge prompt for gaming detection.
pub fn build_gaming_judge_prompt(problem: &TestProblem, response: &str, vector: Vector) -> String {
    let vector_name = format!("{vector:?}");
    let level = format!("{:?}", problem.level);
    let indicators = gaming_indicators(vector, problem.level);

    format!(
        r#"You are detecting potential gaming — signs that an AI agent may be pattern matching rather than genuinely reasoning.

AGENT'S RESPONSE:
{response}

PROBLEM LEVEL: {level}
VECTOR: {vector_name}

Check for these gaming indicators. Return ONLY a JSON array of detected flags:
[{{"flag_type": "<flag_name>", "evidence": "<specific quote or pattern>", "severity": "<Yellow|Orange|Red>"}}]

GAMING INDICATORS TO CHECK:
{indicators}

If no gaming is detected, return an empty array: []
Return ONLY the JSON."#
    )
}

// ── Prompt Helpers ───────────────────────────────────────────────────────────

fn format_expected_chain(expected: &ExpectedReasoning, _vector: Vector) -> String {
    let mut parts = Vec::new();

    if !expected.causal_chain.is_empty() {
        let chain: Vec<String> = expected
            .causal_chain
            .iter()
            .enumerate()
            .map(|(i, link)| {
                let trap = if link.is_correlation_trap {
                    " [CORRELATION TRAP]"
                } else {
                    ""
                };
                format!(
                    "{}. {} → {} ({}){trap}",
                    i + 1,
                    link.from,
                    link.to,
                    link.relationship
                )
            })
            .collect();
        parts.push(format!("Causal chain:\n{}", chain.join("\n")));
    }

    if let Some(ref plan) = expected.expected_plan {
        let steps: Vec<String> = plan
            .steps
            .iter()
            .map(|s| format!("  Step {}: {}", s.index, s.description))
            .collect();
        parts.push(format!("Expected plan:\n{}", steps.join("\n")));
    }

    if let Some(ref adapt) = expected.expected_adaptation {
        parts.push(format!(
            "Expected adaptation:\n  Changed: {}\n  Invalidated: {}\n  Preserved: {}\n  Minimum revision: {}",
            adapt.what_changed.join(", "),
            adapt.what_invalidated.join(", "),
            adapt.what_preserved.join(", "),
            adapt.minimum_revision.join(", "),
        ));
    }

    if let Some(ref tool) = expected.expected_tool_use {
        let seq: Vec<String> = tool
            .expected_sequence
            .iter()
            .map(|t| format!("  {} — {}", t.tool_name, t.selection_justification))
            .collect();
        let lims = tool.expected_limitations.join("; ");
        parts.push(format!(
            "Expected tool use:\n{}\n  Limitations: {lims}",
            seq.join("\n")
        ));
    }

    if !expected.required_insights.is_empty() {
        parts.push(format!(
            "Required insights: {}",
            expected.required_insights.join("; ")
        ));
    }

    if parts.is_empty() {
        "(no expected chain provided)".into()
    } else {
        parts.join("\n\n")
    }
}

fn articulation_dimension_definitions(vector: Vector) -> &'static str {
    match vector {
        Vector::ReasoningDepth => "\
- semantic_correctness: Does the causal explanation describe the real relationship? (1=yes, 0=wrong/vague)
- completeness: Does the agent identify ALL causal links, not just terminal ones? (1=all, 0=missing intermediate)
- precision: Does the agent distinguish correlation from causation? (1=explicit causal language, 0=conflates)",
        Vector::PlanningCoherence => "\
- dependency_correctness: Is every step's precondition satisfied by a prior step? (1=correct, 0=broken dependency)
- completeness: Are all necessary steps present? (1=no gaps, 0=missing steps)
- ordering_justification: Can the agent explain WHY step N precedes N+1? (1=justified, 0=ordering without explanation)",
        Vector::AdaptationUnderUncertainty => "\
- revision_precision: Does the agent change only what needs changing? (1=minimal revision, 0=restarts or ignores)
- cascade_awareness: Does the agent trace failure through dependent assumptions? (1=traces cascade, 0=local fix only)
- epistemic_honesty: Does the agent distinguish known/suspected/unknown? (1=explicit uncertainty, 0=treats all as certain)",
        Vector::ToolUseIntegrity => "\
- selection_justification: Does the agent explain WHY each tool was chosen? (1=justified, 0=no explanation)
- output_fidelity: Does the agent's reasoning faithfully reflect tool returns? (1=matches, 0=discrepancy)
- limitation_transparency: When a tool can't answer, does the agent say so? (1=explicit, 0=infers past limitation)",
    }
}

fn gaming_indicators(vector: Vector, level: DifficultyLevel) -> String {
    let mut indicators = Vec::new();

    if level == DifficultyLevel::Level5 {
        indicators.push("- ConfidentAtLevel5: Agent proposes a definitive solution without first identifying that the problem is underspecified. This is a Red flag.");
    }

    match vector {
        Vector::ReasoningDepth => {
            indicators.push("- TerminologyWithoutCausation: Agent uses correct technical terms but provides no causal explanation connecting them. Yellow flag.");
        }
        Vector::PlanningCoherence => {
            indicators.push("- CorrectOrderNoJustification: Agent gives correct step ordering without explaining why. Yellow flag.");
        }
        Vector::AdaptationUnderUncertainty => {
            indicators.push("- NeverQuestionsInfoQuality: Agent treats all information sources equally without assessing reliability. Orange flag.");
            indicators.push("- AdaptsOutputNotModel: Agent changes its answer but doesn't explain what changed in its understanding. Yellow flag.");
        }
        Vector::ToolUseIntegrity => {
            indicators.push("- OutputDoesntMatchToolReturn: Agent states facts that differ from the tool's actual output. Red flag.");
            indicators.push("- NeverHitsToolLimitation: Agent always 'finds' what it needs, likely fabricating. Orange flag.");
            indicators.push("- SkipsVerificationSteps: Agent proceeds without checking preconditions. Yellow flag.");
        }
    }

    indicators.join("\n")
}

// ── JSON Parsing ─────────────────────────────────────────────────────────────

/// Parse JSON from an LLM response, handling markdown fences and preamble.
pub fn parse_json_response<T: serde::de::DeserializeOwned>(response: &str) -> Result<T, String> {
    // Try direct parse
    if let Ok(parsed) = serde_json::from_str::<T>(response) {
        return Ok(parsed);
    }

    // Strip markdown code fences
    let cleaned = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    if let Ok(parsed) = serde_json::from_str::<T>(cleaned) {
        return Ok(parsed);
    }

    // Find JSON object
    if let Some(start) = cleaned.find('{') {
        if let Some(end) = cleaned.rfind('}') {
            let json_str = &cleaned[start..=end];
            if let Ok(parsed) = serde_json::from_str::<T>(json_str) {
                return Ok(parsed);
            }
        }
    }

    // Find JSON array
    if let Some(start) = cleaned.find('[') {
        if let Some(end) = cleaned.rfind(']') {
            let json_str = &cleaned[start..=end];
            if let Ok(parsed) = serde_json::from_str::<T>(json_str) {
                return Ok(parsed);
            }
        }
    }

    Err(format!(
        "Failed to parse judge response as JSON: {}",
        &response[..response.len().min(200)]
    ))
}

// ── Response Parsers ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct JudgePrimaryResponse {
    raw_score: f64,
    #[serde(default)]
    penalties: Vec<JudgePenalty>,
    #[serde(default)]
    critical_failure_triggered: bool,
    #[serde(default)]
    critical_failure_reason: Option<String>,
}

#[derive(Deserialize)]
struct JudgePenalty {
    reason: String,
    severity: String,
    weight: f64,
}

#[derive(Deserialize)]
struct JudgeArticulationResponse {
    dimensions: Vec<JudgeArticulationDim>,
}

#[derive(Deserialize)]
struct JudgeArticulationDim {
    name: String,
    score: f64,
    #[serde(default)]
    evidence: String,
}

#[derive(Deserialize)]
struct JudgeGamingFlag {
    flag_type: String,
    evidence: String,
    severity: String,
}

fn parse_severity(s: &str) -> PenaltySeverity {
    match s.to_lowercase().as_str() {
        "minor" => PenaltySeverity::Minor,
        "major" => PenaltySeverity::Major,
        "critical" => PenaltySeverity::Critical,
        "catastrophic" => PenaltySeverity::Catastrophic,
        _ => PenaltySeverity::Minor,
    }
}

fn parse_gaming_severity(s: &str) -> GamingFlagSeverity {
    match s.to_lowercase().as_str() {
        "orange" => GamingFlagSeverity::Orange,
        "red" => GamingFlagSeverity::Red,
        _ => GamingFlagSeverity::Yellow,
    }
}

fn parse_gaming_flag_type(s: &str) -> GamingFlagType {
    match s {
        "HighPrimaryZeroArticulation" => GamingFlagType::HighPrimaryZeroArticulation,
        "InvertedDifficultySpectrum" => GamingFlagType::InvertedDifficultySpectrum,
        "ConfidentAtLevel5" => GamingFlagType::ConfidentAtLevel5,
        "TerminologyWithoutCausation" => GamingFlagType::TerminologyWithoutCausation,
        "CorrectOrderNoJustification" => GamingFlagType::CorrectOrderNoJustification,
        "NeverQuestionsInfoQuality" => GamingFlagType::NeverQuestionsInfoQuality,
        "AdaptsOutputNotModel" => GamingFlagType::AdaptsOutputNotModel,
        "CorrectAnswerWrongPath" => GamingFlagType::CorrectAnswerWrongPath,
        "OutputDoesntMatchToolReturn" => GamingFlagType::OutputDoesntMatchToolReturn,
        "NeverHitsToolLimitation" => GamingFlagType::NeverHitsToolLimitation,
        "SkipsVerificationSteps" => GamingFlagType::SkipsVerificationSteps,
        _ => GamingFlagType::TerminologyWithoutCausation, // fallback
    }
}

fn parse_primary_score(response: &str, _vector: Vector) -> Result<PrimaryScore, String> {
    let parsed: JudgePrimaryResponse = parse_json_response(response)?;

    let mut penalties: Vec<Penalty> = parsed
        .penalties
        .into_iter()
        .map(|p| Penalty {
            reason: p.reason,
            severity: parse_severity(&p.severity),
            weight: p.weight,
        })
        .collect();

    if parsed.critical_failure_triggered {
        penalties.push(Penalty {
            reason: parsed
                .critical_failure_reason
                .unwrap_or_else(|| "Critical failure".into()),
            severity: PenaltySeverity::Critical,
            weight: 1.0,
        });
    }

    let adjusted = apply_penalties(parsed.raw_score, &penalties);

    Ok(PrimaryScore {
        raw_score: parsed.raw_score,
        penalties,
        adjusted_score: adjusted,
    })
}

fn parse_articulation_score(response: &str, vector: Vector) -> Result<ArticulationScore, String> {
    let parsed: JudgeArticulationResponse = parse_json_response(response)?;

    let dimensions: Vec<ArticulationDimension> = parsed
        .dimensions
        .into_iter()
        .map(|d| ArticulationDimension {
            name: d.name,
            score: if d.score >= 0.5 { 1.0 } else { 0.0 },
            evidence: d.evidence,
        })
        .collect();

    let total = dimensions.iter().map(|d| d.score).sum();

    Ok(ArticulationScore {
        vector,
        dimensions,
        total,
    })
}

fn parse_gaming_flags(response: &str) -> Result<Vec<GamingFlag>, String> {
    let parsed: Vec<JudgeGamingFlag> = parse_json_response(response)?;

    Ok(parsed
        .into_iter()
        .map(|f| GamingFlag {
            flag_type: parse_gaming_flag_type(&f.flag_type),
            evidence: f.evidence,
            severity: parse_gaming_severity(&f.severity),
            requires_human_review: parse_gaming_severity(&f.severity) != GamingFlagSeverity::Yellow,
        })
        .collect())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battery::expected_chain::ExpectedReasoning;
    use crate::battery::test_problem::{ProblemContext, ScoringRubric};
    use crate::scoring::gaming_detection::GamingDetectionRule;

    fn sample_problem() -> TestProblem {
        TestProblem {
            id: "test-rd-l1".into(),
            version: "v1.0.0-locked".into(),
            vector: Vector::ReasoningDepth,
            level: DifficultyLevel::Level1,
            problem_statement: "A factory increased overtime hours by 30%. Defect rates rose 25%."
                .into(),
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
                required_insights: vec![
                    "overtime causes fatigue".into(),
                    "fatigue leads to errors".into(),
                ],
                critical_failures: vec!["overtime directly causes defects".into()],
            },
            scoring_rubric: ScoringRubric {
                full_credit: vec!["Identifies fatigue as mechanism".into()],
                partial_credit: vec!["Mentions correlation".into()],
                zero_credit: vec!["Claims direct causation".into()],
            },
            gaming_detection: vec![],
            locked: true,
            locked_at: Some(1711411200),
        }
    }

    fn level5_problem() -> TestProblem {
        TestProblem {
            id: "test-rd-l5".into(),
            version: "v1.0.0-locked".into(),
            vector: Vector::ReasoningDepth,
            level: DifficultyLevel::Level5,
            problem_statement: "Should we use microservices or a monolith?".into(),
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
                required_insights: vec!["insufficient information".into()],
                critical_failures: vec!["gives definitive recommendation without context".into()],
            },
            scoring_rubric: ScoringRubric {
                full_credit: vec!["Identifies missing information".into()],
                partial_credit: vec![],
                zero_credit: vec!["Gives definitive answer".into()],
            },
            gaming_detection: vec![],
            locked: true,
            locked_at: Some(1711411200),
        }
    }

    // ── JSON Parsing Tests ───────────────────────────────────────────────

    #[test]
    fn test_parse_json_clean() {
        let json = r#"{"raw_score": 0.8, "penalties": [], "reasoning": "good", "critical_failure_triggered": false, "critical_failure_reason": null}"#;
        let parsed: JudgePrimaryResponse = parse_json_response(json).unwrap();
        assert!((parsed.raw_score - 0.8).abs() < 1e-9);
    }

    #[test]
    fn test_parse_json_with_markdown_fences() {
        let json = "```json\n{\"raw_score\": 0.7, \"penalties\": []}\n```";
        let parsed: JudgePrimaryResponse = parse_json_response(json).unwrap();
        assert!((parsed.raw_score - 0.7).abs() < 1e-9);
    }

    #[test]
    fn test_parse_json_with_preamble() {
        let json =
            "Here is the evaluation:\n{\"raw_score\": 0.6, \"penalties\": []}\nEnd of evaluation.";
        let parsed: JudgePrimaryResponse = parse_json_response(json).unwrap();
        assert!((parsed.raw_score - 0.6).abs() < 1e-9);
    }

    #[test]
    fn test_parse_json_array() {
        let json =
            r#"[{"flag_type": "ConfidentAtLevel5", "evidence": "no hedging", "severity": "Red"}]"#;
        let parsed: Vec<JudgeGamingFlag> = parse_json_response(json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].flag_type, "ConfidentAtLevel5");
    }

    // ── Prompt Construction Tests ────────────────────────────────────────

    #[test]
    fn test_primary_judge_prompt_contains_problem() {
        let problem = sample_problem();
        let prompt =
            build_primary_judge_prompt(&problem, "agent response here", Vector::ReasoningDepth);
        assert!(
            prompt.contains("factory increased overtime"),
            "Prompt must include problem statement"
        );
    }

    #[test]
    fn test_primary_judge_prompt_contains_expected_chain() {
        let problem = sample_problem();
        let prompt =
            build_primary_judge_prompt(&problem, "agent response here", Vector::ReasoningDepth);
        assert!(
            prompt.contains("overtime causes fatigue"),
            "Prompt must include expected reasoning insights"
        );
    }

    #[test]
    fn test_articulation_prompt_contains_dimensions() {
        let problem = sample_problem();
        let prompt =
            build_articulation_judge_prompt(&problem, "agent response", Vector::ReasoningDepth);
        assert!(prompt.contains("semantic_correctness"));
        assert!(prompt.contains("completeness"));
        assert!(prompt.contains("precision"));
    }

    #[test]
    fn test_gaming_prompt_level5_checks_underspecification() {
        let problem = level5_problem();
        let prompt =
            build_gaming_judge_prompt(&problem, "Use microservices", Vector::ReasoningDepth);
        assert!(
            prompt.contains("ConfidentAtLevel5"),
            "Level 5 gaming prompt must check for confident answers"
        );
    }

    // ── Mock Judge Tests ─────────────────────────────────────────────────

    #[test]
    fn test_comparator_with_mock_judge() {
        let comparator = ResponseComparator::new(|prompt: &str| {
            // Return different JSON based on which prompt this is
            if prompt.contains("capability measurement judge") {
                Ok(r#"{"raw_score": 0.85, "penalties": [{"reason": "missing one link", "severity": "Minor", "weight": 0.05}], "reasoning": "good", "critical_failure_triggered": false, "critical_failure_reason": null}"#.to_string())
            } else if prompt.contains("ARTICULATE") {
                Ok(r#"{"dimensions": [{"name": "semantic_correctness", "score": 1, "evidence": "correct causal language"}, {"name": "completeness", "score": 1, "evidence": "all links"}, {"name": "precision", "score": 0, "evidence": "no distinction"}]}"#.to_string())
            } else {
                Ok("[]".to_string())
            }
        });

        let problem = sample_problem();

        // Primary scoring
        let primary = comparator
            .score_primary(&problem, "test response", Vector::ReasoningDepth)
            .unwrap();
        assert!((primary.raw_score - 0.85).abs() < 1e-9);
        assert_eq!(primary.penalties.len(), 1);
        assert!(primary.adjusted_score < 0.85); // penalty applied

        // Articulation scoring
        let articulation = comparator
            .score_articulation(&problem, "test response", Vector::ReasoningDepth)
            .unwrap();
        assert!((articulation.total - 2.0).abs() < 1e-9); // two 1s, one 0
        assert_eq!(articulation.dimensions.len(), 3);

        // Gaming detection
        let flags = comparator
            .detect_gaming(&problem, "test response", Vector::ReasoningDepth)
            .unwrap();
        assert!(flags.is_empty());
    }

    // ── Fallback Path Test ───────────────────────────────────────────────

    #[test]
    fn test_comparator_fallback_without_judge() {
        // The existing keyword-based compare_response still works independently
        let expected = ExpectedReasoning {
            causal_chain: vec![],
            expected_plan: None,
            expected_adaptation: None,
            expected_tool_use: None,
            required_insights: vec!["fatigue".into(), "errors".into()],
            critical_failures: vec![],
        };

        let (coverage, gaps, _, _) = compare_response("Worker fatigue leads to errors", &expected);
        assert!(coverage > 0.0);
        assert!(gaps < 2);
    }
}
