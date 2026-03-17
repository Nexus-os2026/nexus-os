//! Agent testing and scoring — evaluate newly created agents on domain tasks.

use serde::{Deserialize, Serialize};

/// Result of testing a newly created agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub agent_name: String,
    pub tasks: Vec<TestTask>,
    pub average_score: f64,
    pub iterations: u32,
    pub passed: bool,
}

/// A single test task and its result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestTask {
    pub prompt: String,
    pub criteria: Vec<String>,
    pub response_preview: String,
    pub score: f64,
    pub max_score: f64,
}

/// Minimum passing score (out of 10).
pub const MIN_PASSING_SCORE: f64 = 6.0;

/// Maximum iterations to improve a failing agent.
pub const MAX_ITERATIONS: u32 = 3;

/// Build the prompt to generate test tasks for a domain.
pub fn build_test_generation_prompt(
    agent_name: &str,
    capabilities: &[String],
    description: &str,
) -> String {
    format!(
        r#"Generate 3 test tasks that would thoroughly test an AI agent specialized in the following area.

Agent: {agent_name}
Capabilities: {capabilities}
Description: {description}

Each task should have:
- A clear user prompt (the task to give the agent)
- Scoring criteria (3-5 checkpoints that a good response should hit)
- Expected keywords or patterns in a good response

Return ONLY valid JSON in this format:
[
  {{
    "prompt": "the user prompt to test",
    "criteria": ["criterion 1", "criterion 2", "criterion 3"],
    "expected_keywords": ["keyword1", "keyword2"]
  }}
]"#,
        capabilities = capabilities.join(", "),
    )
}

/// Build the prompt to score an agent's response against criteria.
pub fn build_scoring_prompt(
    task_prompt: &str,
    criteria: &[String],
    agent_response: &str,
) -> String {
    let criteria_list: String = criteria
        .iter()
        .enumerate()
        .map(|(i, c)| format!("  {}: \"{}\"", i + 1, c))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"Score this AI agent's response against the given criteria.

Task prompt: "{task_prompt}"

Agent response:
"{response}"

Criteria (score each 0 or 1):
{criteria_list}

Return ONLY valid JSON:
{{
  "scores": [0, 1, 1, 0],
  "total": 2,
  "max": 4,
  "feedback": "brief explanation of what was good/bad"
}}"#,
        response = truncate(agent_response, 2000),
    )
}

/// Build the prompt to improve a system prompt based on test failures.
pub fn build_improvement_prompt(original_prompt: &str, test_results: &[TestTask]) -> String {
    let mut failures = String::new();
    for task in test_results {
        if task.score < task.max_score * 0.7 {
            failures.push_str(&format!(
                "- Task: \"{}\"\n  Score: {}/{}\n  Preview: \"{}\"\n",
                task.prompt,
                task.score,
                task.max_score,
                truncate(&task.response_preview, 200),
            ));
        }
    }

    format!(
        r#"You are improving an AI agent's system prompt based on test failures.

Current system prompt:
"{original_prompt}"

Test failures:
{failures}

Rewrite the system prompt to address these weaknesses. Keep the same format and length (200-400 words).
The improved prompt should make the agent perform better on the failed tasks
while maintaining its strengths on passed tasks.

Return ONLY the improved system prompt text, no JSON wrapping, no code blocks."#,
    )
}

/// Parse test task definitions from LLM response.
pub fn parse_test_tasks(response: &str) -> Result<Vec<TestTaskDef>, String> {
    let json_str = extract_json_array(response)?;
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&json_str).map_err(|e| format!("JSON parse error: {e}"))?;

    let tasks = parsed
        .iter()
        .filter_map(|v| {
            Some(TestTaskDef {
                prompt: v["prompt"].as_str()?.to_string(),
                criteria: v["criteria"]
                    .as_array()?
                    .iter()
                    .filter_map(|c| c.as_str().map(String::from))
                    .collect(),
                expected_keywords: v["expected_keywords"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|k| k.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();

    if tasks.is_empty() {
        return Err("No test tasks parsed from LLM response".to_string());
    }

    Ok(tasks)
}

/// Parse scoring result from LLM response.
pub fn parse_scoring_response(response: &str) -> Result<ScoringResult, String> {
    let json_str = extract_json_object(response)?;
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| format!("Score parse error: {e}"))?;

    Ok(ScoringResult {
        total: parsed["total"].as_f64().unwrap_or(0.0),
        max: parsed["max"].as_f64().unwrap_or(1.0),
        feedback: parsed["feedback"]
            .as_str()
            .unwrap_or("No feedback")
            .to_string(),
    })
}

/// Test task definition from LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestTaskDef {
    pub prompt: String,
    pub criteria: Vec<String>,
    pub expected_keywords: Vec<String>,
}

/// Scoring result from LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringResult {
    pub total: f64,
    pub max: f64,
    pub feedback: String,
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

fn extract_json_array(response: &str) -> Result<String, String> {
    let trimmed = response.trim();

    if trimmed.starts_with('[') {
        return Ok(trimmed.to_string());
    }

    // From code blocks
    if let Some(start) = trimmed.find("```") {
        let after = &trimmed[start + 3..];
        // Skip language identifier
        let after = if let Some(nl) = after.find('\n') {
            &after[nl + 1..]
        } else {
            after
        };
        if let Some(end) = after.find("```") {
            return Ok(after[..end].trim().to_string());
        }
    }

    // Find first [ and last ]
    if let (Some(start), Some(end)) = (trimmed.find('['), trimmed.rfind(']')) {
        if end > start {
            return Ok(trimmed[start..=end].to_string());
        }
    }

    Err("Could not extract JSON array from response".to_string())
}

fn extract_json_object(response: &str) -> Result<String, String> {
    let trimmed = response.trim();

    if trimmed.starts_with('{') {
        return Ok(trimmed.to_string());
    }

    if let Some(start) = trimmed.find("```") {
        let after = &trimmed[start + 3..];
        let after = if let Some(nl) = after.find('\n') {
            &after[nl + 1..]
        } else {
            after
        };
        if let Some(end) = after.find("```") {
            return Ok(after[..end].trim().to_string());
        }
    }

    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if end > start {
            return Ok(trimmed[start..=end].to_string());
        }
    }

    Err("Could not extract JSON object from response".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_test_tasks_basic() {
        let response = r#"[
            {
                "prompt": "Optimize this SQL query",
                "criteria": ["mentions indexing", "identifies full table scan"],
                "expected_keywords": ["index", "scan"]
            }
        ]"#;
        let tasks = parse_test_tasks(response).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].criteria.len(), 2);
    }

    #[test]
    fn parse_scoring_response_basic() {
        let response =
            r#"{"scores": [1, 1, 0], "total": 2, "max": 3, "feedback": "Good coverage"}"#;
        let result = parse_scoring_response(response).unwrap();
        assert!((result.total - 2.0).abs() < f64::EPSILON);
        assert!((result.max - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn build_test_prompt_includes_agent_info() {
        let prompt = build_test_generation_prompt(
            "nexus-dbtuner",
            &["fs.read".to_string()],
            "Database optimization specialist",
        );
        assert!(prompt.contains("nexus-dbtuner"));
        assert!(prompt.contains("Database optimization"));
    }

    #[test]
    fn extract_json_array_from_code_block() {
        let response = "Here are tasks:\n```json\n[{\"prompt\": \"test\"}]\n```";
        let json = extract_json_array(response).unwrap();
        assert!(json.starts_with('['));
    }
}
