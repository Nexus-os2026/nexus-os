//! Task difficulty estimation across the four measurement vectors.

use nexus_capability_measurement::framework::Vector;
use serde::{Deserialize, Serialize};

/// Estimated difficulty for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDifficultyEstimate {
    pub reasoning_difficulty: f64,
    pub planning_difficulty: f64,
    pub adaptation_difficulty: f64,
    pub tool_use_difficulty: f64,
    pub dominant_vector: Vector,
    pub confidence: f64,
    pub method: EstimationMethod,
}

impl TaskDifficultyEstimate {
    pub fn difficulty_for(&self, vector: Vector) -> f64 {
        match vector {
            Vector::ReasoningDepth => self.reasoning_difficulty,
            Vector::PlanningCoherence => self.planning_difficulty,
            Vector::AdaptationUnderUncertainty => self.adaptation_difficulty,
            Vector::ToolUseIntegrity => self.tool_use_difficulty,
        }
    }

    pub fn max_difficulty(&self) -> f64 {
        [
            self.reasoning_difficulty,
            self.planning_difficulty,
            self.adaptation_difficulty,
            self.tool_use_difficulty,
        ]
        .into_iter()
        .fold(f64::MIN, f64::max)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EstimationMethod {
    Heuristic,
    LlmClassification,
    Hybrid,
    Conservative,
}

/// A heuristic rule that maps task patterns to difficulty.
#[derive(Debug, Clone)]
pub struct HeuristicRule {
    pub name: String,
    pub indicators: Vec<String>,
    pub vector: Vector,
    pub difficulty: f64,
    pub weight: f64,
}

/// Type alias for the LLM estimator function.
type LlmEstimatorFn = dyn Fn(&str) -> Result<String, String> + Send + Sync;

/// Estimates task difficulty from the task text.
pub struct DifficultyEstimator {
    heuristic_rules: Vec<HeuristicRule>,
    llm_estimator: Option<Box<LlmEstimatorFn>>,
}

impl Default for DifficultyEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl DifficultyEstimator {
    pub fn new() -> Self {
        Self {
            heuristic_rules: default_heuristics(),
            llm_estimator: None,
        }
    }

    pub fn with_llm(mut self, estimator: Box<LlmEstimatorFn>) -> Self {
        self.llm_estimator = Some(estimator);
        self
    }

    /// Estimate task difficulty from the task text.
    pub fn estimate(&self, task_text: &str) -> TaskDifficultyEstimate {
        let heuristic = self.estimate_heuristic(task_text);

        if let Some(llm) = &self.llm_estimator {
            match self.estimate_llm(task_text, llm.as_ref()) {
                Ok(llm_est) => merge_estimates(heuristic, llm_est),
                Err(_) => heuristic,
            }
        } else {
            heuristic
        }
    }

    fn estimate_heuristic(&self, task_text: &str) -> TaskDifficultyEstimate {
        let lower = task_text.to_lowercase();
        let mut reasoning = 0.0_f64;
        let mut planning = 0.0_f64;
        let mut adaptation = 0.0_f64;
        let mut tool_use = 0.0_f64;
        let mut rw = 0.0_f64;
        let mut pw = 0.0_f64;
        let mut aw = 0.0_f64;
        let mut tw = 0.0_f64;
        let mut total_weight = 0.0_f64;

        for rule in &self.heuristic_rules {
            let matches = rule
                .indicators
                .iter()
                .any(|ind| lower.contains(&ind.to_lowercase()));
            if matches {
                match rule.vector {
                    Vector::ReasoningDepth => {
                        reasoning += rule.difficulty * rule.weight;
                        rw += rule.weight;
                    }
                    Vector::PlanningCoherence => {
                        planning += rule.difficulty * rule.weight;
                        pw += rule.weight;
                    }
                    Vector::AdaptationUnderUncertainty => {
                        adaptation += rule.difficulty * rule.weight;
                        aw += rule.weight;
                    }
                    Vector::ToolUseIntegrity => {
                        tool_use += rule.difficulty * rule.weight;
                        tw += rule.weight;
                    }
                }
                total_weight += rule.weight;
            }
        }

        if total_weight > 0.0 {
            if rw > 0.0 {
                reasoning /= rw;
            }
            if pw > 0.0 {
                planning /= pw;
            }
            if aw > 0.0 {
                adaptation /= aw;
            }
            if tw > 0.0 {
                tool_use /= tw;
            }
        } else {
            reasoning = 0.5;
            planning = 0.5;
            adaptation = 0.5;
            tool_use = 0.5;
        }

        let scores = [
            (Vector::ReasoningDepth, reasoning),
            (Vector::PlanningCoherence, planning),
            (Vector::AdaptationUnderUncertainty, adaptation),
            (Vector::ToolUseIntegrity, tool_use),
        ];
        let dominant = scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(v, _)| *v)
            .unwrap_or(Vector::ReasoningDepth);

        let confidence = if total_weight > 3.0 {
            0.7
        } else if total_weight > 1.0 {
            0.5
        } else {
            0.3
        };

        TaskDifficultyEstimate {
            reasoning_difficulty: reasoning.clamp(0.0, 1.0),
            planning_difficulty: planning.clamp(0.0, 1.0),
            adaptation_difficulty: adaptation.clamp(0.0, 1.0),
            tool_use_difficulty: tool_use.clamp(0.0, 1.0),
            dominant_vector: dominant,
            confidence,
            method: if total_weight > 0.0 {
                EstimationMethod::Heuristic
            } else {
                EstimationMethod::Conservative
            },
        }
    }

    fn estimate_llm(
        &self,
        task_text: &str,
        llm: &LlmEstimatorFn,
    ) -> Result<TaskDifficultyEstimate, String> {
        let prompt = format!(
            r#"Estimate the difficulty of this task. Return ONLY JSON:
{{"reasoning_difficulty":<0-1>,"planning_difficulty":<0-1>,"adaptation_difficulty":<0-1>,"tool_use_difficulty":<0-1>,"dominant_vector":"<ReasoningDepth|PlanningCoherence|AdaptationUnderUncertainty|ToolUseIntegrity>"}}

TASK: {task_text}"#
        );
        let response = llm(&prompt)?;
        parse_llm_estimate(&response)
    }
}

fn parse_llm_estimate(response: &str) -> Result<TaskDifficultyEstimate, String> {
    let cleaned = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    #[derive(Deserialize)]
    struct Raw {
        reasoning_difficulty: f64,
        planning_difficulty: f64,
        adaptation_difficulty: f64,
        tool_use_difficulty: f64,
        dominant_vector: String,
    }

    let parsed: Raw = serde_json::from_str(cleaned)
        .or_else(|_| {
            if let Some(start) = cleaned.find('{') {
                if let Some(end) = cleaned.rfind('}') {
                    return serde_json::from_str(&cleaned[start..=end]);
                }
            }
            Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "No JSON found",
            )))
        })
        .map_err(|e| format!("Parse error: {e}"))?;

    let dominant = match parsed.dominant_vector.as_str() {
        "PlanningCoherence" => Vector::PlanningCoherence,
        "AdaptationUnderUncertainty" => Vector::AdaptationUnderUncertainty,
        "ToolUseIntegrity" => Vector::ToolUseIntegrity,
        _ => Vector::ReasoningDepth,
    };

    Ok(TaskDifficultyEstimate {
        reasoning_difficulty: parsed.reasoning_difficulty.clamp(0.0, 1.0),
        planning_difficulty: parsed.planning_difficulty.clamp(0.0, 1.0),
        adaptation_difficulty: parsed.adaptation_difficulty.clamp(0.0, 1.0),
        tool_use_difficulty: parsed.tool_use_difficulty.clamp(0.0, 1.0),
        dominant_vector: dominant,
        confidence: 0.8,
        method: EstimationMethod::LlmClassification,
    })
}

fn merge_estimates(
    heuristic: TaskDifficultyEstimate,
    llm: TaskDifficultyEstimate,
) -> TaskDifficultyEstimate {
    let lw = 0.7;
    let hw = 0.3;
    TaskDifficultyEstimate {
        reasoning_difficulty: llm.reasoning_difficulty * lw + heuristic.reasoning_difficulty * hw,
        planning_difficulty: llm.planning_difficulty * lw + heuristic.planning_difficulty * hw,
        adaptation_difficulty: llm.adaptation_difficulty * lw
            + heuristic.adaptation_difficulty * hw,
        tool_use_difficulty: llm.tool_use_difficulty * lw + heuristic.tool_use_difficulty * hw,
        dominant_vector: llm.dominant_vector,
        confidence: (llm.confidence * lw + heuristic.confidence * hw).clamp(0.0, 1.0),
        method: EstimationMethod::Hybrid,
    }
}

fn default_heuristics() -> Vec<HeuristicRule> {
    vec![
        // Reasoning
        HeuristicRule {
            name: "causal".into(),
            indicators: vec![
                "why".into(),
                "cause".into(),
                "because".into(),
                "root cause".into(),
                "diagnose".into(),
            ],
            vector: Vector::ReasoningDepth,
            difficulty: 0.5,
            weight: 1.0,
        },
        HeuristicRule {
            name: "constraint_conflict".into(),
            indicators: vec![
                "conflict".into(),
                "tradeoff".into(),
                "constraint".into(),
                "impossible".into(),
            ],
            vector: Vector::ReasoningDepth,
            difficulty: 0.7,
            weight: 1.5,
        },
        HeuristicRule {
            name: "underspecified".into(),
            indicators: vec![
                "what should".into(),
                "make it better".into(),
                "optimize".into(),
            ],
            vector: Vector::ReasoningDepth,
            difficulty: 0.9,
            weight: 2.0,
        },
        // Planning
        HeuristicRule {
            name: "simple_seq".into(),
            indicators: vec![
                "deploy".into(),
                "install".into(),
                "setup".into(),
                "configure".into(),
            ],
            vector: Vector::PlanningCoherence,
            difficulty: 0.3,
            weight: 1.0,
        },
        HeuristicRule {
            name: "multi_phase".into(),
            indicators: vec!["migrate".into(), "rollback".into(), "zero downtime".into()],
            vector: Vector::PlanningCoherence,
            difficulty: 0.7,
            weight: 1.5,
        },
        HeuristicRule {
            name: "circular".into(),
            indicators: vec![
                "circular".into(),
                "deadlock".into(),
                "dependency cycle".into(),
            ],
            vector: Vector::PlanningCoherence,
            difficulty: 0.85,
            weight: 2.0,
        },
        // Adaptation
        HeuristicRule {
            name: "changing".into(),
            indicators: vec![
                "changed".into(),
                "updated".into(),
                "new requirement".into(),
                "pivot".into(),
            ],
            vector: Vector::AdaptationUnderUncertainty,
            difficulty: 0.5,
            weight: 1.0,
        },
        HeuristicRule {
            name: "conflicting".into(),
            indicators: vec![
                "contradicts".into(),
                "inconsistent".into(),
                "conflicting".into(),
            ],
            vector: Vector::AdaptationUnderUncertainty,
            difficulty: 0.7,
            weight: 1.5,
        },
        HeuristicRule {
            name: "adversarial".into(),
            indicators: vec!["suspicious".into(), "untrusted".into(), "phishing".into()],
            vector: Vector::AdaptationUnderUncertainty,
            difficulty: 0.9,
            weight: 2.0,
        },
        // Tool use
        HeuristicRule {
            name: "single_tool".into(),
            indicators: vec![
                "read file".into(),
                "list".into(),
                "search".into(),
                "query".into(),
            ],
            vector: Vector::ToolUseIntegrity,
            difficulty: 0.2,
            weight: 1.0,
        },
        HeuristicRule {
            name: "tool_chain".into(),
            indicators: vec!["then use".into(), "chain".into(), "pipeline".into()],
            vector: Vector::ToolUseIntegrity,
            difficulty: 0.5,
            weight: 1.0,
        },
        HeuristicRule {
            name: "tool_limit".into(),
            indicators: vec![
                "can you".into(),
                "is it possible".into(),
                "determine".into(),
            ],
            vector: Vector::ToolUseIntegrity,
            difficulty: 0.7,
            weight: 1.5,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_difficulty_estimator_heuristic() {
        let est = DifficultyEstimator::new();
        let result = est.estimate("Diagnose the root cause of the deployment failure");
        assert!(
            result.reasoning_difficulty > 0.3,
            "Root cause analysis should indicate reasoning difficulty"
        );
        assert!(matches!(result.method, EstimationMethod::Heuristic));
    }

    #[test]
    fn test_difficulty_estimator_conservative_default() {
        let est = DifficultyEstimator::new();
        let result = est.estimate("xyzzy gibberish no keywords match");
        assert!((result.reasoning_difficulty - 0.5).abs() < 1e-9);
        assert!((result.planning_difficulty - 0.5).abs() < 1e-9);
        assert!((result.adaptation_difficulty - 0.5).abs() < 1e-9);
        assert!((result.tool_use_difficulty - 0.5).abs() < 1e-9);
        assert!(matches!(result.method, EstimationMethod::Conservative));
    }

    #[test]
    fn test_planning_keywords() {
        let est = DifficultyEstimator::new();
        let result = est.estimate("Plan a zero downtime migration with rollback");
        assert!(result.planning_difficulty > result.tool_use_difficulty);
    }
}
