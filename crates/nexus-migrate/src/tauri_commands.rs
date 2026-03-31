use crate::crewai::CrewAIParser;
use crate::langgraph::LangGraphParser;
use crate::output::OutputGenerator;

/// Preview migration without committing (dry run).
///
/// Returns the `MigrationResult` as JSON — agents, tasks, workflows,
/// warnings, errors, and summary.
pub fn migrate_preview(
    source: &str,
    agents_yaml: Option<&str>,
    tasks_yaml: Option<&str>,
    python_source: Option<&str>,
) -> Result<serde_json::Value, String> {
    let result = match source.to_lowercase().as_str() {
        "crewai" => {
            let agents = agents_yaml.ok_or("CrewAI migration requires agents_yaml")?;
            CrewAIParser::migrate(agents, tasks_yaml).map_err(|e| e.to_string())?
        }
        "langgraph" => {
            let python = python_source.ok_or("LangGraph migration requires python_source")?;
            LangGraphParser::migrate(python).map_err(|e| e.to_string())?
        }
        other => {
            return Err(format!(
                "Unsupported source framework: '{other}'. Supported: crewai, langgraph"
            ))
        }
    };

    serde_json::to_value(&result).map_err(|e| format!("Serialization error: {e}"))
}

/// Execute migration and generate output files.
///
/// Returns the `MigrationOutput` as JSON — agent genomes, workflow configs,
/// and a human-readable report.
pub fn migrate_execute(
    source: &str,
    agents_yaml: Option<&str>,
    tasks_yaml: Option<&str>,
    python_source: Option<&str>,
) -> Result<serde_json::Value, String> {
    let result = match source.to_lowercase().as_str() {
        "crewai" => {
            let agents = agents_yaml.ok_or("CrewAI migration requires agents_yaml")?;
            CrewAIParser::migrate(agents, tasks_yaml).map_err(|e| e.to_string())?
        }
        "langgraph" => {
            let python = python_source.ok_or("LangGraph migration requires python_source")?;
            LangGraphParser::migrate(python).map_err(|e| e.to_string())?
        }
        other => {
            return Err(format!(
                "Unsupported source framework: '{other}'. Supported: crewai, langgraph"
            ))
        }
    };

    let output = OutputGenerator::generate_all(&result);
    serde_json::to_value(&output).map_err(|e| format!("Serialization error: {e}"))
}

/// Get list of supported source frameworks.
pub fn migrate_supported_sources() -> Vec<String> {
    vec!["crewai".into(), "langgraph".into()]
}

/// Get a human-readable migration report from a preview result.
pub fn migrate_report(
    source: &str,
    agents_yaml: Option<&str>,
    tasks_yaml: Option<&str>,
    python_source: Option<&str>,
) -> Result<String, String> {
    let result = match source.to_lowercase().as_str() {
        "crewai" => {
            let agents = agents_yaml.ok_or("CrewAI migration requires agents_yaml")?;
            CrewAIParser::migrate(agents, tasks_yaml).map_err(|e| e.to_string())?
        }
        "langgraph" => {
            let python = python_source.ok_or("LangGraph migration requires python_source")?;
            LangGraphParser::migrate(python).map_err(|e| e.to_string())?
        }
        other => return Err(format!("Unsupported source framework: '{other}'")),
    };

    Ok(OutputGenerator::generate_report(&result))
}

// Convenience types for Tauri command signatures.
// These mirror the shapes in types.rs but as plain JSON for IPC.

/// Generate Tauri #[command] wrappers for the desktop backend.
///
/// In the desktop backend (`main.rs`), register these as:
///
/// ```rust,ignore
/// #[tauri::command]
/// fn migrate_preview_cmd(
///     source: String,
///     agents_yaml: Option<String>,
///     tasks_yaml: Option<String>,
///     python_source: Option<String>,
/// ) -> Result<serde_json::Value, String> {
///     nexus_migrate::tauri_commands::migrate_preview(
///         &source,
///         agents_yaml.as_deref(),
///         tasks_yaml.as_deref(),
///         python_source.as_deref(),
///     )
/// }
/// ```
pub fn _command_signatures() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_crewai() {
        let agents = r#"
writer:
  role: "Writer"
  goal: "Write articles"
"#;
        let result = migrate_preview("crewai", Some(agents), None, None).unwrap();
        assert!(result["summary"]["agents_converted"].as_u64().unwrap() > 0);
    }

    #[test]
    fn test_preview_langgraph() {
        let python = r#"
graph = StateGraph(State)
graph.add_node("agent", call_model)
graph.set_entry_point("agent")
graph.add_edge("agent", END)
"#;
        let result = migrate_preview("langgraph", None, None, Some(python)).unwrap();
        assert!(result["summary"]["workflows_converted"].as_u64().unwrap() > 0);
    }

    #[test]
    fn test_execute_crewai() {
        let agents = r#"
writer:
  role: "Writer"
  goal: "Write articles"
"#;
        let result = migrate_execute("crewai", Some(agents), None, None).unwrap();
        assert!(result["agents"].is_array());
        assert!(result["report"].is_string());
    }

    #[test]
    fn test_unsupported_source() {
        let result = migrate_preview("pytorch", None, None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported"));
    }

    #[test]
    fn test_supported_sources() {
        let sources = migrate_supported_sources();
        assert!(sources.contains(&"crewai".to_string()));
        assert!(sources.contains(&"langgraph".to_string()));
    }

    #[test]
    fn test_report_generation() {
        let agents = r#"
writer:
  role: "Writer"
  goal: "Write articles"
  tools:
    - SerperDevTool
"#;
        let report = migrate_report("crewai", Some(agents), None, None).unwrap();
        assert!(report.contains("Migration Report"));
        assert!(report.contains("writer"));
    }

    #[test]
    fn test_crewai_missing_agents_yaml() {
        let result = migrate_preview("crewai", None, None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires agents_yaml"));
    }

    #[test]
    fn test_langgraph_missing_python() {
        let result = migrate_preview("langgraph", None, None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires python_source"));
    }
}
