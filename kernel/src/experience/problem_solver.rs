//! Problem Solver — analyse real business/life problems and propose automated solutions.
//!
//! The user describes a pain point in plain language.  The solver produces an
//! analysis with root causes, estimated cost of the problem, and a concrete
//! buildable solution with ROI projections.

use serde::{Deserialize, Serialize};

/// Lightweight profile so the solver can tailor recommendations.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserProfile {
    pub name: Option<String>,
    pub business_type: Option<String>,
    pub team_size: Option<u32>,
    pub tools_in_use: Vec<String>,
}

/// A concrete solution that Nexus can build.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedSolution {
    pub title: String,
    pub features: Vec<String>,
    pub build_time_minutes: u32,
    pub monthly_cost: String,
    pub expected_savings: String,
}

/// Full analysis of a user's problem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemAnalysis {
    pub problem_summary: String,
    pub root_causes: Vec<String>,
    pub current_cost: String,
    pub solution: ProposedSolution,
    pub buildable: bool,
}

/// Stateless problem solver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemSolver;

impl Default for ProblemSolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ProblemSolver {
    pub fn new() -> Self {
        Self
    }

    /// Produce a problem analysis from the user's description and LLM output.
    ///
    /// `llm_json` is expected to be a JSON object with keys:
    /// `problem_summary`, `root_causes`, `current_cost`, `solution_title`,
    /// `solution_features`, `build_time_minutes`, `monthly_cost`,
    /// `expected_savings`, `buildable`.
    pub fn analyze(&self, _problem: &str, llm_json: &str) -> ProblemAnalysis {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(llm_json) {
            ProblemAnalysis {
                problem_summary: v["problem_summary"]
                    .as_str()
                    .unwrap_or(_problem)
                    .to_string(),
                root_causes: v["root_causes"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                current_cost: v["current_cost"].as_str().unwrap_or("unknown").to_string(),
                solution: ProposedSolution {
                    title: v["solution_title"]
                        .as_str()
                        .unwrap_or("Automated solution")
                        .to_string(),
                    features: v["solution_features"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    build_time_minutes: v["build_time_minutes"].as_u64().unwrap_or(20) as u32,
                    monthly_cost: v["monthly_cost"].as_str().unwrap_or("$0").to_string(),
                    expected_savings: v["expected_savings"]
                        .as_str()
                        .unwrap_or("significant time savings")
                        .to_string(),
                },
                buildable: v["buildable"].as_bool().unwrap_or(true),
            }
        } else {
            // Fallback: produce a basic analysis from the raw text.
            ProblemAnalysis {
                problem_summary: _problem.to_string(),
                root_causes: vec![llm_json.to_string()],
                current_cost: "unknown".to_string(),
                solution: ProposedSolution {
                    title: "Automated solution".to_string(),
                    features: vec!["Custom automation".to_string()],
                    build_time_minutes: 20,
                    monthly_cost: "$0".to_string(),
                    expected_savings: "significant time savings".to_string(),
                },
                buildable: true,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_from_json() {
        let solver = ProblemSolver::new();
        let llm = r#"{
            "problem_summary": "Slow email response time",
            "root_causes": ["Manual reading", "No templates"],
            "current_cost": "45 min/day",
            "solution_title": "Auto-Responder Bot",
            "solution_features": ["Auto-answer FAQs", "Draft complex replies"],
            "build_time_minutes": 20,
            "monthly_cost": "$0",
            "expected_savings": "Save 40 min/day",
            "buildable": true
        }"#;
        let analysis = solver.analyze("slow email response", llm);
        assert_eq!(analysis.problem_summary, "Slow email response time");
        assert_eq!(analysis.root_causes.len(), 2);
        assert_eq!(analysis.solution.title, "Auto-Responder Bot");
        assert!(analysis.buildable);
    }

    #[test]
    fn test_analyze_fallback() {
        let solver = ProblemSolver::new();
        let analysis = solver.analyze("too much manual work", "not json at all");
        assert_eq!(analysis.problem_summary, "too much manual work");
        assert!(analysis.buildable);
    }
}
