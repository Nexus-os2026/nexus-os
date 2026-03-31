use std::collections::HashMap;

use serde_json::json;
use uuid::Uuid;

use crate::tool_map::{collect_capabilities, map_crewai_tool, map_llm_config};
use crate::types::{
    ConvertedAgent, ConvertedTask, ConvertedTool, MigrateError, MigrationError, MigrationResult,
    MigrationSummary, MigrationWarning, SourceFramework,
};

// ── Intermediate types for YAML parsing ─────────────────────────────────

/// Raw agent definition from a CrewAI agents.yaml file.
#[derive(Debug, Clone)]
pub struct CrewAIAgent {
    pub name: String,
    pub role: String,
    pub goal: String,
    pub backstory: String,
    pub tools: Vec<String>,
    pub llm: Option<String>,
    pub verbose: bool,
    pub allow_delegation: bool,
    pub max_iter: Option<u32>,
    pub max_rpm: Option<u32>,
    pub reasoning: bool,
    pub extra_fields: HashMap<String, serde_json::Value>,
}

/// Raw task definition from a CrewAI tasks.yaml file.
#[derive(Debug, Clone)]
pub struct CrewAITask {
    pub name: String,
    pub description: String,
    pub expected_output: Option<String>,
    pub agent: Option<String>,
    pub context: Vec<String>,
    pub output_file: Option<String>,
    pub extra_fields: HashMap<String, serde_json::Value>,
}

// ── Parser ──────────────────────────────────────────────────────────────

pub struct CrewAIParser;

impl CrewAIParser {
    /// Parse a CrewAI `agents.yaml` file.
    pub fn parse_agents(yaml_content: &str) -> Result<Vec<CrewAIAgent>, MigrateError> {
        let raw: HashMap<String, serde_yaml::Value> = serde_yaml::from_str(yaml_content)
            .map_err(|e| MigrateError::YamlParse(format!("agents.yaml: {e}")))?;

        let mut agents = Vec::with_capacity(raw.len());
        for (name, value) in &raw {
            let map = match value.as_mapping() {
                Some(m) => m,
                None => continue,
            };
            let get_str = |key: &str| -> String {
                map.get(serde_yaml::Value::String(key.to_string()))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            let get_bool = |key: &str, default: bool| -> bool {
                map.get(serde_yaml::Value::String(key.to_string()))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(default)
            };
            let get_u32 = |key: &str| -> Option<u32> {
                map.get(serde_yaml::Value::String(key.to_string()))
                    .and_then(|v| v.as_u64())
                    .and_then(|v| u32::try_from(v).ok())
            };

            let tools: Vec<String> = map
                .get(serde_yaml::Value::String("tools".to_string()))
                .and_then(|v| v.as_sequence())
                .map(|seq| {
                    seq.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let llm = map
                .get(serde_yaml::Value::String("llm".to_string()))
                .and_then(|v| v.as_str())
                .map(String::from);

            // Collect extra fields that we didn't explicitly handle.
            let known_keys = [
                "role",
                "goal",
                "backstory",
                "tools",
                "llm",
                "verbose",
                "allow_delegation",
                "max_iter",
                "max_rpm",
                "reasoning",
            ];
            let mut extra = HashMap::new();
            for (k, v) in map.iter() {
                if let Some(key_str) = k.as_str() {
                    if !known_keys.contains(&key_str) {
                        if let Ok(jv) = serde_json::to_value(v) {
                            extra.insert(key_str.to_string(), jv);
                        }
                    }
                }
            }

            agents.push(CrewAIAgent {
                name: name.clone(),
                role: get_str("role"),
                goal: get_str("goal"),
                backstory: get_str("backstory"),
                tools,
                llm,
                verbose: get_bool("verbose", false),
                allow_delegation: get_bool("allow_delegation", false),
                max_iter: get_u32("max_iter"),
                max_rpm: get_u32("max_rpm"),
                reasoning: get_bool("reasoning", false),
                extra_fields: extra,
            });
        }

        // Sort by name for deterministic output.
        agents.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(agents)
    }

    /// Parse a CrewAI `tasks.yaml` file.
    pub fn parse_tasks(yaml_content: &str) -> Result<Vec<CrewAITask>, MigrateError> {
        let raw: HashMap<String, serde_yaml::Value> = serde_yaml::from_str(yaml_content)
            .map_err(|e| MigrateError::YamlParse(format!("tasks.yaml: {e}")))?;

        let mut tasks = Vec::with_capacity(raw.len());
        for (name, value) in &raw {
            let map = match value.as_mapping() {
                Some(m) => m,
                None => continue,
            };
            let get_str = |key: &str| -> String {
                map.get(serde_yaml::Value::String(key.to_string()))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            let get_opt_str = |key: &str| -> Option<String> {
                map.get(serde_yaml::Value::String(key.to_string()))
                    .and_then(|v| v.as_str())
                    .map(String::from)
            };

            let context: Vec<String> = map
                .get(serde_yaml::Value::String("context".to_string()))
                .and_then(|v| v.as_sequence())
                .map(|seq| {
                    seq.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let known_keys = [
                "description",
                "expected_output",
                "agent",
                "context",
                "output_file",
            ];
            let mut extra = HashMap::new();
            for (k, v) in map.iter() {
                if let Some(key_str) = k.as_str() {
                    if !known_keys.contains(&key_str) {
                        if let Ok(jv) = serde_json::to_value(v) {
                            extra.insert(key_str.to_string(), jv);
                        }
                    }
                }
            }

            tasks.push(CrewAITask {
                name: name.clone(),
                description: get_str("description"),
                expected_output: get_opt_str("expected_output"),
                agent: get_opt_str("agent"),
                context,
                output_file: get_opt_str("output_file"),
                extra_fields: extra,
            });
        }

        tasks.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tasks)
    }

    /// Parse CrewAI files and convert to Nexus OS format.
    pub fn migrate(
        agents_yaml: &str,
        tasks_yaml: Option<&str>,
    ) -> Result<MigrationResult, MigrateError> {
        let crew_agents = Self::parse_agents(agents_yaml)?;
        let crew_tasks = match tasks_yaml {
            Some(yaml) => Self::parse_tasks(yaml)?,
            None => Vec::new(),
        };

        let total_agents_found = crew_agents.len();
        let total_tasks_found = crew_tasks.len();

        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        // ── Convert agents ──
        let mut converted_agents = Vec::new();
        // Map from original crew agent name → generated nexus agent id.
        let mut agent_id_map: HashMap<String, String> = HashMap::new();

        for ca in &crew_agents {
            match convert_agent(ca, &mut warnings) {
                Ok(agent) => {
                    agent_id_map.insert(ca.name.clone(), agent.nexus_agent_id.clone());
                    converted_agents.push(agent);
                }
                Err(msg) => {
                    errors.push(MigrationError {
                        item: ca.name.clone(),
                        message: msg,
                        original_content: None,
                    });
                }
            }
        }

        // ── Convert tasks ──
        let mut converted_tasks = Vec::new();
        for ct in &crew_tasks {
            match convert_task(ct, &agent_id_map, &mut warnings) {
                Ok(task) => converted_tasks.push(task),
                Err(msg) => {
                    errors.push(MigrationError {
                        item: ct.name.clone(),
                        message: msg,
                        original_content: None,
                    });
                }
            }
        }

        let summary = MigrationSummary {
            source_framework: SourceFramework::CrewAI,
            total_agents_found,
            agents_converted: converted_agents.len(),
            total_tasks_found,
            tasks_converted: converted_tasks.len(),
            total_workflows_found: 0,
            workflows_converted: 0,
            warnings_count: warnings.len(),
            errors_count: errors.len(),
        };

        Ok(MigrationResult {
            source_framework: SourceFramework::CrewAI,
            agents_converted: converted_agents,
            tasks_converted: converted_tasks,
            workflows_converted: Vec::new(),
            warnings,
            errors,
            summary,
        })
    }
}

// ── Conversion helpers ──────────────────────────────────────────────────

fn convert_agent(
    ca: &CrewAIAgent,
    warnings: &mut Vec<MigrationWarning>,
) -> Result<ConvertedAgent, String> {
    if ca.role.is_empty() && ca.goal.is_empty() {
        return Err(format!(
            "Agent '{}' has no role and no goal — cannot convert",
            ca.name
        ));
    }

    let nexus_id = format!("nexus-{}", ca.name.replace('_', "-").to_lowercase());

    let tools: Vec<ConvertedTool> = ca.tools.iter().map(|t| map_crewai_tool(t)).collect();

    // Emit warnings for unmapped tools.
    for tool in &tools {
        if !tool.mapped {
            warnings.push(MigrationWarning {
                item: format!("{} → {}", ca.name, tool.original_name),
                message: format!(
                    "Tool '{}' has no direct Nexus OS equivalent",
                    tool.original_name
                ),
                suggestion: format!(
                    "Create a custom capability '{}' or map to an existing one",
                    tool.nexus_capability
                ),
            });
        }
    }

    // Check for API key requirements.
    for tool in &tools {
        if tool.original_name == "SerperDevTool" {
            warnings.push(MigrationWarning {
                item: ca.name.clone(),
                message: "SerperDevTool requires SERPER_API_KEY".into(),
                suggestion: "Configure the web search API key in Nexus OS settings".into(),
            });
        }
    }

    let capabilities = collect_capabilities(&tools);

    let (llm_provider, llm_model) = ca
        .llm
        .as_deref()
        .map(map_llm_config)
        .unwrap_or((None, None));

    let autonomy_level: u8 = if ca.allow_delegation { 4 } else { 3 };

    // Build the full Nexus OS genome JSON.
    let mut config = json!({});
    if ca.verbose {
        config["verbose"] = json!(true);
    }
    if let Some(max_iter) = ca.max_iter {
        config["max_iterations"] = json!(max_iter);
    }
    if let Some(max_rpm) = ca.max_rpm {
        config["max_rpm"] = json!(max_rpm);
    }
    if ca.reasoning {
        config["enable_planning"] = json!(true);
    }
    if !ca.extra_fields.is_empty() {
        config["crewai_extra"] = json!(ca.extra_fields);
    }

    let description = format!(
        "Migrated from CrewAI agent '{}'. Role: {}. Goal: {}. {}",
        ca.name, ca.role, ca.goal, ca.backstory
    );

    let genome = json!({
        "name": nexus_id,
        "version": "1.0.0",
        "description": description,
        "capabilities": capabilities,
        "autonomy_level": autonomy_level,
        "fuel_budget": 10000,
        "default_goal": ca.goal,
        "llm_model": llm_model.as_deref().unwrap_or("auto"),
        "migrated_from": {
            "framework": "CrewAI",
            "original_name": ca.name,
            "config": config,
        }
    });

    Ok(ConvertedAgent {
        original_name: ca.name.clone(),
        nexus_agent_id: nexus_id,
        role: ca.role.clone(),
        goal: ca.goal.clone(),
        backstory: ca.backstory.clone(),
        autonomy_level,
        capabilities,
        llm_provider,
        llm_model,
        tools,
        genome,
    })
}

fn convert_task(
    ct: &CrewAITask,
    agent_id_map: &HashMap<String, String>,
    warnings: &mut Vec<MigrationWarning>,
) -> Result<ConvertedTask, String> {
    if ct.description.is_empty() {
        return Err(format!(
            "Task '{}' has no description — cannot convert",
            ct.name
        ));
    }

    let task_id = Uuid::new_v4().to_string();

    let assigned_agent =
        ct.agent
            .as_ref()
            .and_then(|agent_name| match agent_id_map.get(agent_name) {
                Some(id) => Some(id.clone()),
                None => {
                    warnings.push(MigrationWarning {
                        item: ct.name.clone(),
                        message: format!(
                            "Task references agent '{}' which was not found in agents.yaml",
                            agent_name
                        ),
                        suggestion: "Assign this task to an existing Nexus OS agent after import"
                            .into(),
                    });
                    None
                }
            });

    let dependencies: Vec<String> = ct.context.clone();

    Ok(ConvertedTask {
        original_name: ct.name.clone(),
        nexus_task_id: task_id,
        description: ct.description.clone(),
        expected_output: ct.expected_output.clone(),
        assigned_agent,
        dependencies,
    })
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const AGENTS_YAML: &str = r#"
researcher:
  role: "{topic} Senior Data Researcher"
  goal: "Uncover cutting-edge developments in {topic}"
  backstory: "You're a seasoned researcher with a knack for uncovering the latest developments."
  verbose: true
  tools:
    - SerperDevTool
    - ScrapeWebsiteTool
  llm: gpt-4o
  allow_delegation: false
  max_iter: 20
  reasoning: true

reporting_analyst:
  role: "{topic} Reporting Analyst"
  goal: "Create detailed reports based on {topic} data"
  backstory: "You're a meticulous analyst known for thorough reports."
  tools:
    - FileWriteTool
  llm: claude-3-opus
  allow_delegation: true
"#;

    const TASKS_YAML: &str = r#"
research_task:
  description: "Conduct thorough research about {topic}"
  expected_output: "A list with 10 bullet points"
  agent: researcher
  output_file: research.md

reporting_task:
  description: "Review the context and expand into a full report"
  expected_output: "A fully fledged report"
  agent: reporting_analyst
  context:
    - research_task
  output_file: report.md
"#;

    #[test]
    fn test_parse_agents_basic() {
        let agents = CrewAIParser::parse_agents(AGENTS_YAML).unwrap();
        assert_eq!(agents.len(), 2);
    }

    #[test]
    fn test_parse_agents_fields() {
        let agents = CrewAIParser::parse_agents(AGENTS_YAML).unwrap();
        let researcher = agents.iter().find(|a| a.name == "researcher").unwrap();
        assert_eq!(researcher.role, "{topic} Senior Data Researcher");
        assert_eq!(
            researcher.goal,
            "Uncover cutting-edge developments in {topic}"
        );
        assert!(researcher.verbose);
        assert!(!researcher.allow_delegation);
        assert_eq!(researcher.max_iter, Some(20));
        assert!(researcher.reasoning);
        assert_eq!(researcher.tools.len(), 2);
        assert_eq!(researcher.llm.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn test_parse_agents_template_vars_preserved() {
        let agents = CrewAIParser::parse_agents(AGENTS_YAML).unwrap();
        let researcher = agents.iter().find(|a| a.name == "researcher").unwrap();
        assert!(researcher.role.contains("{topic}"));
        assert!(researcher.goal.contains("{topic}"));
    }

    #[test]
    fn test_parse_tasks_basic() {
        let tasks = CrewAIParser::parse_tasks(TASKS_YAML).unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_parse_tasks_dependencies() {
        let tasks = CrewAIParser::parse_tasks(TASKS_YAML).unwrap();
        let reporting = tasks.iter().find(|t| t.name == "reporting_task").unwrap();
        assert_eq!(reporting.context, vec!["research_task"]);
        assert_eq!(reporting.agent.as_deref(), Some("reporting_analyst"));
    }

    #[test]
    fn test_migrate_full() {
        let result = CrewAIParser::migrate(AGENTS_YAML, Some(TASKS_YAML)).unwrap();
        assert_eq!(result.summary.agents_converted, 2);
        assert_eq!(result.summary.tasks_converted, 2);
        assert_eq!(result.source_framework, SourceFramework::CrewAI);
    }

    #[test]
    fn test_migrate_agent_conversion() {
        let result = CrewAIParser::migrate(AGENTS_YAML, Some(TASKS_YAML)).unwrap();
        let researcher = result
            .agents_converted
            .iter()
            .find(|a| a.original_name == "researcher")
            .unwrap();
        assert_eq!(researcher.nexus_agent_id, "nexus-researcher");
        assert_eq!(researcher.autonomy_level, 3); // allow_delegation: false
        assert_eq!(researcher.llm_provider.as_deref(), Some("openai"));
        assert_eq!(researcher.llm_model.as_deref(), Some("gpt-4o"));
        assert!(researcher.capabilities.contains(&"web.search".to_string()));
        assert!(researcher.capabilities.contains(&"web.fetch".to_string()));
    }

    #[test]
    fn test_migrate_delegation_autonomy() {
        let result = CrewAIParser::migrate(AGENTS_YAML, Some(TASKS_YAML)).unwrap();
        let analyst = result
            .agents_converted
            .iter()
            .find(|a| a.original_name == "reporting_analyst")
            .unwrap();
        assert_eq!(analyst.autonomy_level, 4); // allow_delegation: true
        assert_eq!(analyst.llm_provider.as_deref(), Some("anthropic"));
    }

    #[test]
    fn test_migrate_genome_json() {
        let result = CrewAIParser::migrate(AGENTS_YAML, None).unwrap();
        let researcher = result
            .agents_converted
            .iter()
            .find(|a| a.original_name == "researcher")
            .unwrap();
        let genome = &researcher.genome;
        assert_eq!(genome["name"], "nexus-researcher");
        assert_eq!(genome["version"], "1.0.0");
        assert_eq!(genome["autonomy_level"], 3);
        assert!(genome["description"].as_str().unwrap().contains("CrewAI"));
        assert_eq!(genome["llm_model"], "gpt-4o");
    }

    #[test]
    fn test_migrate_task_agent_mapping() {
        let result = CrewAIParser::migrate(AGENTS_YAML, Some(TASKS_YAML)).unwrap();
        let research_task = result
            .tasks_converted
            .iter()
            .find(|t| t.original_name == "research_task")
            .unwrap();
        assert_eq!(
            research_task.assigned_agent.as_deref(),
            Some("nexus-researcher")
        );

        let reporting_task = result
            .tasks_converted
            .iter()
            .find(|t| t.original_name == "reporting_task")
            .unwrap();
        assert_eq!(
            reporting_task.assigned_agent.as_deref(),
            Some("nexus-reporting-analyst")
        );
        assert_eq!(reporting_task.dependencies, vec!["research_task"]);
    }

    #[test]
    fn test_migrate_agents_only_no_tasks() {
        let result = CrewAIParser::migrate(AGENTS_YAML, None).unwrap();
        assert_eq!(result.summary.agents_converted, 2);
        assert_eq!(result.summary.tasks_converted, 0);
    }

    #[test]
    fn test_malformed_yaml_produces_error() {
        let bad_yaml = "this: is: not: valid: yaml: [[[";
        let err = CrewAIParser::parse_agents(bad_yaml);
        assert!(err.is_err());
        match err.unwrap_err() {
            MigrateError::YamlParse(msg) => assert!(msg.contains("agents.yaml")),
            other => panic!("Expected YamlParse, got {other:?}"),
        }
    }

    #[test]
    fn test_empty_agent_fields_graceful() {
        let yaml = r#"
empty_agent:
  role: ""
  goal: "Do something"
"#;
        let result = CrewAIParser::migrate(yaml, None).unwrap();
        assert_eq!(result.summary.agents_converted, 1);
    }

    #[test]
    fn test_agent_no_role_no_goal_produces_error() {
        let yaml = r#"
broken_agent:
  verbose: true
"#;
        let result = CrewAIParser::migrate(yaml, None).unwrap();
        assert_eq!(result.summary.agents_converted, 0);
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_task_references_missing_agent_warns() {
        let yaml_agents = r#"
writer:
  role: "Writer"
  goal: "Write things"
"#;
        let yaml_tasks = r#"
write_task:
  description: "Write a report"
  agent: nonexistent_agent
"#;
        let result = CrewAIParser::migrate(yaml_agents, Some(yaml_tasks)).unwrap();
        assert_eq!(result.summary.tasks_converted, 1);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("nonexistent_agent")));
        let task = &result.tasks_converted[0];
        assert!(task.assigned_agent.is_none());
    }

    #[test]
    fn test_serper_tool_api_key_warning() {
        let yaml = r#"
searcher:
  role: "Searcher"
  goal: "Search the web"
  tools:
    - SerperDevTool
"#;
        let result = CrewAIParser::migrate(yaml, None).unwrap();
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("SERPER_API_KEY")));
    }

    #[test]
    fn test_task_empty_description_error() {
        let yaml_agents = r#"
agent1:
  role: "R"
  goal: "G"
"#;
        let yaml_tasks = r#"
bad_task:
  expected_output: "something"
"#;
        let result = CrewAIParser::migrate(yaml_agents, Some(yaml_tasks)).unwrap();
        assert_eq!(result.summary.tasks_converted, 0);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].message.contains("no description"));
    }
}
