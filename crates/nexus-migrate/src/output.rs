use serde_json::json;

use crate::types::{ConvertedWorkflow, MigrationOutput, MigrationResult};

pub struct OutputGenerator;

impl OutputGenerator {
    /// Generate a Nexus OS agent genome JSON from a converted agent.
    pub fn generate_agent_genome(agent: &crate::types::ConvertedAgent) -> serde_json::Value {
        agent.genome.clone()
    }

    /// Generate a Nexus OS workflow config from a converted workflow.
    pub fn generate_workflow_config(workflow: &ConvertedWorkflow) -> serde_json::Value {
        let nodes: Vec<serde_json::Value> = workflow
            .nodes
            .iter()
            .map(|n| {
                json!({
                    "id": n.id,
                    "type": format!("{:?}", n.node_type),
                    "agent_id": n.agent_id,
                    "description": n.description,
                })
            })
            .collect();

        let edges: Vec<serde_json::Value> = workflow
            .edges
            .iter()
            .map(|e| {
                json!({
                    "from": e.from_node,
                    "to": e.to_node,
                    "condition": e.condition,
                })
            })
            .collect();

        json!({
            "id": workflow.nexus_workflow_id,
            "name": workflow.original_name,
            "entry_point": workflow.entry_point,
            "nodes": nodes,
            "edges": edges,
            "state_schema": workflow.state_schema,
            "migrated_from": {
                "framework": "LangGraph",
                "original_name": workflow.original_name,
            }
        })
    }

    /// Generate a complete migration output with all agents, tasks, and workflows.
    pub fn generate_all(result: &MigrationResult) -> MigrationOutput {
        let agents: Vec<serde_json::Value> = result
            .agents_converted
            .iter()
            .map(Self::generate_agent_genome)
            .collect();

        let workflows: Vec<serde_json::Value> = result
            .workflows_converted
            .iter()
            .map(Self::generate_workflow_config)
            .collect();

        let report = Self::generate_report(result);

        MigrationOutput {
            agents,
            workflows,
            report,
        }
    }

    /// Generate a human-readable migration report.
    pub fn generate_report(result: &MigrationResult) -> String {
        let mut lines = Vec::new();

        lines.push("=== Migration Report ===".to_string());
        lines.push(format!("Source: {}", result.source_framework));
        lines.push(String::new());

        // Agents
        if result.summary.total_agents_found > 0 {
            lines.push(format!(
                "Agents Converted: {}/{}",
                result.summary.agents_converted, result.summary.total_agents_found
            ));
            for agent in &result.agents_converted {
                let caps = agent.capabilities.join(", ");
                lines.push(format!(
                    "  \u{2705} {} \u{2192} {} (L{}, capabilities: {})",
                    agent.original_name, agent.nexus_agent_id, agent.autonomy_level, caps
                ));
            }
            lines.push(String::new());
        }

        // Tasks
        if result.summary.total_tasks_found > 0 {
            lines.push(format!(
                "Tasks Converted: {}/{}",
                result.summary.tasks_converted, result.summary.total_tasks_found
            ));
            for task in &result.tasks_converted {
                let assigned = task.assigned_agent.as_deref().unwrap_or("unassigned");
                let deps = if task.dependencies.is_empty() {
                    String::new()
                } else {
                    format!(" (depends on: {})", task.dependencies.join(", "))
                };
                lines.push(format!(
                    "  \u{2705} {} \u{2192} assigned to {}{}",
                    task.original_name, assigned, deps
                ));
            }
            lines.push(String::new());
        }

        // Workflows
        if result.summary.total_workflows_found > 0 {
            lines.push(format!(
                "Workflows Converted: {}/{}",
                result.summary.workflows_converted, result.summary.total_workflows_found
            ));
            for wf in &result.workflows_converted {
                let node_count = wf.nodes.len().saturating_sub(2); // exclude __start__/__end__
                let edge_count = wf.edges.len();
                lines.push(format!(
                    "  \u{2705} {} \u{2192} {} ({} nodes, {} edges, entry: {})",
                    wf.original_name, wf.nexus_workflow_id, node_count, edge_count, wf.entry_point
                ));
            }
            lines.push(String::new());
        }

        // Warnings
        if !result.warnings.is_empty() {
            lines.push(format!("Warnings: {}", result.warnings.len()));
            for w in &result.warnings {
                lines.push(format!("  \u{26A0}\u{FE0F} [{}] {}", w.item, w.message));
                lines.push(format!("     Suggestion: {}", w.suggestion));
            }
            lines.push(String::new());
        }

        // Errors
        if !result.errors.is_empty() {
            lines.push(format!("Errors: {}", result.errors.len()));
            for e in &result.errors {
                lines.push(format!("  \u{274C} [{}] {}", e.item, e.message));
            }
            lines.push(String::new());
        }

        // Next steps
        lines.push("Next Steps:".to_string());
        lines.push("1. Review generated agent genomes".to_string());
        lines.push("2. Configure LLM providers in Nexus OS settings".to_string());
        lines.push("3. Import agents via the Agent Store or create_agent API".to_string());
        if !result.workflows_converted.is_empty() {
            lines.push("4. Import workflows via the Workflow editor".to_string());
        }
        if result
            .warnings
            .iter()
            .any(|w| w.message.contains("API_KEY"))
        {
            lines.push(format!(
                "{}. Ensure required API keys are configured",
                if result.workflows_converted.is_empty() {
                    4
                } else {
                    5
                }
            ));
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crewai::CrewAIParser;
    use crate::langgraph::LangGraphParser;

    const AGENTS_YAML: &str = r#"
researcher:
  role: "Senior Researcher"
  goal: "Find data"
  backstory: "Expert researcher"
  tools:
    - SerperDevTool
  llm: gpt-4o
"#;

    const TASKS_YAML: &str = r#"
research_task:
  description: "Do research"
  expected_output: "A report"
  agent: researcher
"#;

    #[test]
    fn test_generate_agent_genome_valid_json() {
        let result = CrewAIParser::migrate(AGENTS_YAML, None).unwrap();
        let genome = OutputGenerator::generate_agent_genome(&result.agents_converted[0]);
        assert!(genome.is_object());
        assert!(genome["name"].is_string());
        assert!(genome["capabilities"].is_array());
    }

    #[test]
    fn test_generate_all_agents() {
        let result = CrewAIParser::migrate(AGENTS_YAML, Some(TASKS_YAML)).unwrap();
        let output = OutputGenerator::generate_all(&result);
        assert_eq!(output.agents.len(), 1);
        assert!(!output.report.is_empty());
    }

    #[test]
    fn test_generate_report_readable() {
        let result = CrewAIParser::migrate(AGENTS_YAML, Some(TASKS_YAML)).unwrap();
        let report = OutputGenerator::generate_report(&result);
        assert!(report.contains("Migration Report"));
        assert!(report.contains("Agents Converted: 1/1"));
        assert!(report.contains("Tasks Converted: 1/1"));
        assert!(report.contains("researcher"));
    }

    #[test]
    fn test_report_includes_warnings() {
        let result = CrewAIParser::migrate(AGENTS_YAML, None).unwrap();
        let report = OutputGenerator::generate_report(&result);
        assert!(report.contains("Warnings:"));
        assert!(report.contains("SERPER_API_KEY"));
    }

    #[test]
    fn test_generate_workflow_config() {
        let python = r#"
class State(TypedDict):
    messages: list[str]

graph = StateGraph(State)
graph.add_node("agent", call_model)
graph.add_edge("agent", END)
graph.set_entry_point("agent")
"#;
        let result = LangGraphParser::migrate(python).unwrap();
        let wf_config = OutputGenerator::generate_workflow_config(&result.workflows_converted[0]);
        assert!(wf_config["nodes"].is_array());
        assert!(wf_config["edges"].is_array());
        assert_eq!(wf_config["entry_point"], "agent");
    }

    #[test]
    fn test_summary_counts_accurate() {
        let result = CrewAIParser::migrate(AGENTS_YAML, Some(TASKS_YAML)).unwrap();
        assert_eq!(result.summary.agents_converted, 1);
        assert_eq!(result.summary.tasks_converted, 1);
        assert_eq!(result.summary.total_agents_found, 1);
        assert_eq!(result.summary.total_tasks_found, 1);

        let output = OutputGenerator::generate_all(&result);
        assert_eq!(output.agents.len(), result.summary.agents_converted);
    }

    #[test]
    fn test_report_with_errors() {
        let yaml_agents = r#"
broken:
  verbose: true
"#;
        let result = CrewAIParser::migrate(yaml_agents, None).unwrap();
        let report = OutputGenerator::generate_report(&result);
        assert!(report.contains("Errors:"));
    }
}
