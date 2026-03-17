//! Capability gap detection — analyze user requests against the existing agent pool.

use serde::{Deserialize, Serialize};

use super::generator::AgentSpec;

/// Result of analyzing a user request for capability gaps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapAnalysis {
    pub user_request: String,
    pub required_capabilities: Vec<String>,
    pub closest_existing_agents: Vec<AgentMatch>,
    pub missing_capabilities: Vec<String>,
    pub gap_found: bool,
    pub recommended_agent_spec: Option<AgentSpec>,
}

/// A scored match between an existing agent and the user request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMatch {
    pub agent_id: String,
    pub relevance_score: f64,
    pub matching_capabilities: Vec<String>,
    pub missing_capabilities: Vec<String>,
}

/// Lightweight summary of an existing agent for gap comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistingAgentSummary {
    pub name: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub autonomy_level: u32,
}

/// Build the LLM prompt for gap analysis.
pub fn build_gap_analysis_prompt(user_request: &str, agents: &[ExistingAgentSummary]) -> String {
    let mut agent_list = String::new();
    for a in agents {
        agent_list.push_str(&format!(
            "- {} (L{}): capabilities=[{}], description=\"{}\"\n",
            a.name,
            a.autonomy_level,
            a.capabilities.join(", "),
            truncate(&a.description, 200),
        ));
    }

    format!(
        r#"You are a capability gap analyzer for an AI agent operating system called Nexus OS.

The user requested: "{user_request}"

Available agents and their capabilities:
{agent_list}

Analyze:
1. What capabilities does this request require? (list as keywords)
2. Which existing agents are the closest matches? (top 3, with relevance score 0.0–1.0)
3. What capabilities are MISSING that no existing agent covers well?
4. Should a new agent be created? (true/false)

If a new agent should be created, specify:
- name: nexus-<descriptive-name> (lowercase, hyphenated)
- display_name: human-readable name
- description: one-sentence purpose
- category: one of [coding, security, data, creative, devops, research, communication, productivity, specialized]
- capabilities: list of required Nexus OS capabilities from [web.search, web.read, llm.query, fs.read, fs.write, process.exec, mcp.call]
- autonomy_level: 1-5 (1=reactive, 3=autonomous, 5=full autonomy)
- tools: list of tools the agent needs
- reasoning_strategy: one of [direct, chain_of_thought, tree_of_thought, react]
- temperature: 0.0-1.0

Return ONLY valid JSON in this exact format:
{{
  "required_capabilities": ["cap1", "cap2"],
  "closest_agents": [
    {{"agent_id": "nexus-name", "relevance_score": 0.5, "matching_capabilities": ["cap1"], "missing_capabilities": ["cap2"]}}
  ],
  "missing_capabilities": ["cap2"],
  "gap_found": true,
  "recommended_spec": {{
    "name": "nexus-newagent",
    "display_name": "New Agent",
    "description": "Purpose of the agent",
    "category": "specialized",
    "capabilities": ["fs.read", "fs.write"],
    "autonomy_level": 3,
    "tools": ["fs.read", "fs.write"],
    "reasoning_strategy": "chain_of_thought",
    "temperature": 0.7
  }}
}}

If no gap is found, set gap_found to false and omit recommended_spec."#,
    )
}

/// Parse the LLM response into a `GapAnalysis`.
pub fn parse_gap_analysis_response(
    user_request: &str,
    response: &str,
) -> Result<GapAnalysis, String> {
    // Extract JSON from response (handle markdown code blocks)
    let json_str = extract_json(response)?;

    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| format!("JSON parse error: {e}"))?;

    let required_capabilities = parsed["required_capabilities"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let closest_existing_agents = parsed["closest_agents"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    Some(AgentMatch {
                        agent_id: v["agent_id"].as_str()?.to_string(),
                        relevance_score: v["relevance_score"].as_f64().unwrap_or(0.0),
                        matching_capabilities: v["matching_capabilities"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        missing_capabilities: v["missing_capabilities"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let missing_capabilities = parsed["missing_capabilities"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let gap_found = parsed["gap_found"].as_bool().unwrap_or(false);

    let recommended_agent_spec = if gap_found {
        parsed.get("recommended_spec").and_then(|spec| {
            Some(AgentSpec {
                name: spec["name"].as_str()?.to_string(),
                display_name: spec["display_name"].as_str().unwrap_or("").to_string(),
                description: spec["description"].as_str().unwrap_or("").to_string(),
                system_prompt: String::new(), // Generated in step 2
                autonomy_level: spec["autonomy_level"].as_u64().unwrap_or(3) as u32,
                capabilities: spec["capabilities"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                tools: spec["tools"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                category: spec["category"]
                    .as_str()
                    .unwrap_or("specialized")
                    .to_string(),
                reasoning_strategy: spec["reasoning_strategy"]
                    .as_str()
                    .unwrap_or("chain_of_thought")
                    .to_string(),
                temperature: spec["temperature"].as_f64().unwrap_or(0.7),
                parent_agents: Vec::new(),
            })
        })
    } else {
        None
    };

    Ok(GapAnalysis {
        user_request: user_request.to_string(),
        required_capabilities,
        closest_existing_agents,
        missing_capabilities,
        gap_found,
        recommended_agent_spec,
    })
}

/// Extract JSON from a response that may include markdown code fences.
fn extract_json(response: &str) -> Result<String, String> {
    let trimmed = response.trim();

    // Try direct parse first
    if trimmed.starts_with('{') {
        return Ok(trimmed.to_string());
    }

    // Extract from ```json ... ``` blocks
    if let Some(start) = trimmed.find("```json") {
        let after = &trimmed[start + 7..];
        if let Some(end) = after.find("```") {
            return Ok(after[..end].trim().to_string());
        }
    }

    // Extract from ``` ... ``` blocks
    if let Some(start) = trimmed.find("```") {
        let after = &trimmed[start + 3..];
        if let Some(end) = after.find("```") {
            let inner = after[..end].trim();
            // Skip language identifier line if present
            let json_part = if let Some(newline) = inner.find('\n') {
                let first_line = &inner[..newline];
                if first_line.chars().all(|c| c.is_alphanumeric()) {
                    &inner[newline + 1..]
                } else {
                    inner
                }
            } else {
                inner
            };
            return Ok(json_part.trim().to_string());
        }
    }

    // Last resort: find first { and last }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if end > start {
            return Ok(trimmed[start..=end].to_string());
        }
    }

    Err(format!(
        "Could not extract JSON from LLM response: {}",
        truncate(trimmed, 200)
    ))
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gap_found_response() {
        let response = r#"{
            "required_capabilities": ["3d_rendering", "game_logic"],
            "closest_agents": [
                {"agent_id": "nexus-forge", "relevance_score": 0.4, "matching_capabilities": ["coding"], "missing_capabilities": ["3d_rendering"]}
            ],
            "missing_capabilities": ["3d_rendering", "game_logic"],
            "gap_found": true,
            "recommended_spec": {
                "name": "nexus-gamewright",
                "display_name": "GameWright",
                "description": "3D game design specialist",
                "category": "creative",
                "capabilities": ["fs.read", "fs.write"],
                "autonomy_level": 3,
                "tools": ["fs.read", "fs.write"],
                "reasoning_strategy": "chain_of_thought",
                "temperature": 0.8
            }
        }"#;

        let analysis = parse_gap_analysis_response("Design a 3D game", response).unwrap();
        assert!(analysis.gap_found);
        assert_eq!(analysis.missing_capabilities.len(), 2);
        assert!(analysis.recommended_agent_spec.is_some());
        let spec = analysis.recommended_agent_spec.unwrap();
        assert_eq!(spec.name, "nexus-gamewright");
    }

    #[test]
    fn parse_no_gap_response() {
        let response = r#"{
            "required_capabilities": ["code_review"],
            "closest_agents": [
                {"agent_id": "nexus-codesentry", "relevance_score": 0.95, "matching_capabilities": ["code_review"], "missing_capabilities": []}
            ],
            "missing_capabilities": [],
            "gap_found": false
        }"#;

        let analysis = parse_gap_analysis_response("Review Python code", response).unwrap();
        assert!(!analysis.gap_found);
        assert!(analysis.recommended_agent_spec.is_none());
    }

    #[test]
    fn extract_json_from_code_block() {
        let response = "Here is the analysis:\n```json\n{\"gap_found\": true}\n```";
        let json = extract_json(response).unwrap();
        assert_eq!(json, "{\"gap_found\": true}");
    }

    #[test]
    fn extract_json_direct() {
        let response = "{\"gap_found\": false}";
        let json = extract_json(response).unwrap();
        assert_eq!(json, "{\"gap_found\": false}");
    }

    #[test]
    fn build_prompt_includes_agents() {
        let agents = vec![ExistingAgentSummary {
            name: "nexus-forge".to_string(),
            description: "Code generation agent".to_string(),
            capabilities: vec!["fs.read".to_string()],
            autonomy_level: 3,
        }];
        let prompt = build_gap_analysis_prompt("help me code", &agents);
        assert!(prompt.contains("nexus-forge"));
        assert!(prompt.contains("help me code"));
    }
}
