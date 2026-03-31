use regex::Regex;
use serde_json::json;
use uuid::Uuid;

use crate::types::{
    ConvertedWorkflow, MigrateError, MigrationError, MigrationResult, MigrationSummary,
    MigrationWarning, SourceFramework, WorkflowEdge, WorkflowNode, WorkflowNodeType,
};

// ── Intermediate types ──────────────────────────────────────────────────

/// A LangGraph graph definition extracted from Python source.
#[derive(Debug, Clone)]
pub struct LangGraphDefinition {
    pub graph_var: String,
    pub state_class: String,
    pub nodes: Vec<(String, String)>, // (node_name, function_name)
    pub edges: Vec<(String, String)>, // (from, to)  — "END" is a special value
    pub conditional_edges: Vec<ConditionalEdge>,
    pub entry_point: Option<String>,
    pub state_fields: Vec<(String, String)>, // (field_name, type_hint)
}

#[derive(Debug, Clone)]
pub struct ConditionalEdge {
    pub source: String,
    pub function_name: String,
    pub routes: Vec<(String, String)>, // (condition_key, target_node)
}

// ── Parser ──────────────────────────────────────────────────────────────

pub struct LangGraphParser;

impl LangGraphParser {
    /// Parse a Python file containing LangGraph graph definitions.
    ///
    /// This is a best-effort regex-based parser. It catches common patterns
    /// but won't handle every possible LangGraph configuration.
    pub fn parse_python(python_content: &str) -> Result<Vec<LangGraphDefinition>, MigrateError> {
        let mut definitions = Vec::new();

        // 1. Find all StateGraph instantiations
        let graph_re = Regex::new(r"(\w+)\s*=\s*StateGraph\((\w+)\)")
            .map_err(|e| MigrateError::PythonParse(format!("regex error: {e}")))?;

        for cap in graph_re.captures_iter(python_content) {
            let graph_var = cap[1].to_string();
            let state_class = cap[2].to_string();

            let mut def = LangGraphDefinition {
                graph_var: graph_var.clone(),
                state_class: state_class.clone(),
                nodes: Vec::new(),
                edges: Vec::new(),
                conditional_edges: Vec::new(),
                entry_point: None,
                state_fields: Vec::new(),
            };

            // 2. Extract nodes: graph_var.add_node("name", func)
            let node_re = Regex::new(&format!(
                r#"{graph_var}\.add_node\(\s*["'](\w+)["']\s*,\s*(\w+)"#
            ))
            .map_err(|e| MigrateError::PythonParse(format!("regex error: {e}")))?;

            for cap in node_re.captures_iter(python_content) {
                def.nodes.push((cap[1].to_string(), cap[2].to_string()));
            }

            // 3. Extract edges: graph_var.add_edge("from", "to") or .add_edge("from", END)
            //    Also handle START → node patterns.
            let edge_re = Regex::new(&format!(
                r#"{graph_var}\.add_edge\(\s*(?:["'](\w+)["']|(\w+))\s*,\s*(?:["'](\w+)["']|(\w+))\s*\)"#
            ))
            .map_err(|e| MigrateError::PythonParse(format!("regex error: {e}")))?;

            for cap in edge_re.captures_iter(python_content) {
                let from = cap
                    .get(1)
                    .or_else(|| cap.get(2))
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();
                let to = cap
                    .get(3)
                    .or_else(|| cap.get(4))
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();
                if !from.is_empty() && !to.is_empty() {
                    def.edges.push((from, to));
                }
            }

            // 4. Extract entry point: graph_var.set_entry_point("node")
            let entry_re = Regex::new(&format!(
                r#"{graph_var}\.set_entry_point\(\s*["'](\w+)["']\s*\)"#
            ))
            .map_err(|e| MigrateError::PythonParse(format!("regex error: {e}")))?;

            if let Some(cap) = entry_re.captures(python_content) {
                def.entry_point = Some(cap[1].to_string());
            }

            // 5. Extract conditional edges (basic pattern)
            let cond_re = Regex::new(&format!(
                r#"{graph_var}\.add_conditional_edges\(\s*["'](\w+)["']\s*,\s*(\w+)"#
            ))
            .map_err(|e| MigrateError::PythonParse(format!("regex error: {e}")))?;

            for cap in cond_re.captures_iter(python_content) {
                let source = cap[1].to_string();
                let func = cap[2].to_string();

                // Try to extract the routing dict if present in the same statement.
                let routes = extract_routing_dict(python_content, &source, &graph_var);

                def.conditional_edges.push(ConditionalEdge {
                    source,
                    function_name: func,
                    routes,
                });
            }

            // 6. Extract state schema if we can find the TypedDict or BaseModel class.
            def.state_fields = extract_state_fields(python_content, &state_class);

            definitions.push(def);
        }

        Ok(definitions)
    }

    /// Convert parsed LangGraph definitions to Nexus OS format.
    pub fn migrate(python_content: &str) -> Result<MigrationResult, MigrateError> {
        let definitions = Self::parse_python(python_content)?;
        let total_workflows_found = definitions.len();
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        let mut converted_workflows = Vec::new();

        for def in &definitions {
            match convert_workflow(def, &mut warnings) {
                Ok(wf) => converted_workflows.push(wf),
                Err(msg) => {
                    errors.push(MigrationError {
                        item: def.graph_var.clone(),
                        message: msg,
                        original_content: None,
                    });
                }
            }
        }

        if definitions.is_empty() {
            warnings.push(MigrationWarning {
                item: "source".into(),
                message: "No StateGraph definitions found in the Python source".into(),
                suggestion: "Ensure the file contains `variable = StateGraph(StateName)` patterns"
                    .into(),
            });
        }

        let summary = MigrationSummary {
            source_framework: SourceFramework::LangGraph,
            total_agents_found: 0,
            agents_converted: 0,
            total_tasks_found: 0,
            tasks_converted: 0,
            total_workflows_found,
            workflows_converted: converted_workflows.len(),
            warnings_count: warnings.len(),
            errors_count: errors.len(),
        };

        Ok(MigrationResult {
            source_framework: SourceFramework::LangGraph,
            agents_converted: Vec::new(),
            tasks_converted: Vec::new(),
            workflows_converted: converted_workflows,
            warnings,
            errors,
            summary,
        })
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Try to extract a routing dict from `add_conditional_edges(...)`.
///
/// Looks for patterns like `{"continue": "tools", "end": END}`.
fn extract_routing_dict(
    content: &str,
    source_node: &str,
    graph_var: &str,
) -> Vec<(String, String)> {
    // Build a regex that captures everything inside the conditional_edges call
    // for this specific source node.
    let pattern = format!(
        r#"{graph_var}\.add_conditional_edges\(\s*["']{source_node}["'][^)]*\{{\s*([^}}]+)\}}"#
    );
    let re = match Regex::new(&pattern) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let Some(cap) = re.captures(content) else {
        return Vec::new();
    };

    let dict_body = &cap[1];
    let mut routes = Vec::new();

    // Parse simple "key": "value" or "key": END pairs.
    let pair_re = match Regex::new(r#"["'](\w+)["']\s*:\s*(?:["'](\w+)["']|(\w+))"#) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    for pair_cap in pair_re.captures_iter(dict_body) {
        let key = pair_cap[1].to_string();
        let value = pair_cap
            .get(2)
            .or_else(|| pair_cap.get(3))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "END".to_string());
        routes.push((key, value));
    }

    routes
}

/// Try to extract state fields from a TypedDict or Pydantic BaseModel class.
fn extract_state_fields(content: &str, class_name: &str) -> Vec<(String, String)> {
    // TypedDict pattern: class ClassName(TypedDict):
    let typed_dict_pattern = format!(r"class\s+{class_name}\s*\(\s*TypedDict\s*\)\s*:");
    let pydantic_pattern = format!(r"class\s+{class_name}\s*\(\s*BaseModel\s*\)\s*:");

    let class_re = match Regex::new(&format!("(?:{typed_dict_pattern})|(?:{pydantic_pattern})")) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let Some(class_match) = class_re.find(content) else {
        return Vec::new();
    };

    let rest = &content[class_match.end()..];
    let mut fields = Vec::new();

    // Field pattern: `field_name: type_hint`
    let field_re = match Regex::new(r"^\s+(\w+)\s*:\s*(.+?)(?:\s*=.*)?$") {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    for line in rest.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Stop at the next class/function definition or unindented line.
        if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
            break;
        }
        if let Some(cap) = field_re.captures(line) {
            fields.push((cap[1].to_string(), cap[2].trim().to_string()));
        }
    }

    fields
}

/// Classify a node function as an agent execution, tool call, etc.
fn classify_node(func_name: &str) -> WorkflowNodeType {
    let lower = func_name.to_lowercase();
    if lower.contains("tool") || lower.contains("execute") || lower.contains("action") {
        WorkflowNodeType::ToolCall
    } else if lower.contains("human") || lower.contains("review") || lower.contains("approve") {
        WorkflowNodeType::HumanReview
    } else if lower.contains("route") || lower.contains("decide") || lower.contains("should") {
        WorkflowNodeType::ConditionalBranch
    } else {
        WorkflowNodeType::AgentExecution
    }
}

fn convert_workflow(
    def: &LangGraphDefinition,
    warnings: &mut Vec<MigrationWarning>,
) -> Result<ConvertedWorkflow, String> {
    if def.nodes.is_empty() && def.edges.is_empty() {
        return Err(format!(
            "Graph '{}' has no nodes or edges — cannot convert",
            def.graph_var
        ));
    }

    let workflow_id = Uuid::new_v4().to_string();

    // Convert nodes.
    let mut nodes: Vec<WorkflowNode> = def
        .nodes
        .iter()
        .map(|(name, func)| WorkflowNode {
            id: name.clone(),
            node_type: classify_node(func),
            agent_id: None,
            description: format!("LangGraph node '{name}' (function: {func})"),
        })
        .collect();

    // Add __start__ and __end__ sentinel nodes.
    nodes.push(WorkflowNode {
        id: "__start__".into(),
        node_type: WorkflowNodeType::Start,
        agent_id: None,
        description: "Workflow entry point".into(),
    });
    nodes.push(WorkflowNode {
        id: "__end__".into(),
        node_type: WorkflowNodeType::End,
        agent_id: None,
        description: "Workflow completion".into(),
    });

    // Convert edges.
    let mut edges: Vec<WorkflowEdge> = Vec::new();

    for (from, to) in &def.edges {
        let to_normalized = if to == "END" || to == "__end__" {
            "__end__".to_string()
        } else if to == "START" || to == "__start__" {
            "__start__".to_string()
        } else {
            to.clone()
        };
        let from_normalized = if from == "START" || from == "__start__" {
            "__start__".to_string()
        } else {
            from.clone()
        };
        edges.push(WorkflowEdge {
            from_node: from_normalized,
            to_node: to_normalized,
            condition: None,
        });
    }

    // Convert conditional edges.
    for cond in &def.conditional_edges {
        if cond.routes.is_empty() {
            warnings.push(MigrationWarning {
                item: format!("{} → conditional_edges({})", def.graph_var, cond.source),
                message: format!(
                    "Could not parse routing dict for conditional edge from '{}' via '{}'",
                    cond.source, cond.function_name
                ),
                suggestion:
                    "Manually define the conditional transitions in the Nexus OS workflow editor"
                        .into(),
            });
            // Still add a placeholder edge.
            edges.push(WorkflowEdge {
                from_node: cond.source.clone(),
                to_node: "__end__".into(),
                condition: Some(format!("{}()", cond.function_name)),
            });
        } else {
            for (key, target) in &cond.routes {
                let to = if target == "END" || target == "__end__" {
                    "__end__".to_string()
                } else {
                    target.clone()
                };
                edges.push(WorkflowEdge {
                    from_node: cond.source.clone(),
                    to_node: to,
                    condition: Some(format!("{}() == \"{}\"", cond.function_name, key)),
                });
            }
        }
    }

    // Add entry edge if we have an entry point.
    let entry_point = def
        .entry_point
        .clone()
        .or_else(|| def.nodes.first().map(|(n, _)| n.clone()))
        .unwrap_or_else(|| "__start__".into());

    if entry_point != "__start__" {
        // Add an edge from __start__ to the entry point if not already present.
        let already_has = edges
            .iter()
            .any(|e| e.from_node == "__start__" && e.to_node == entry_point);
        if !already_has {
            edges.push(WorkflowEdge {
                from_node: "__start__".into(),
                to_node: entry_point.clone(),
                condition: None,
            });
        }
    }

    // Build state schema.
    let state_schema = if def.state_fields.is_empty() {
        json!({
            "class": def.state_class,
            "fields": {}
        })
    } else {
        let fields: serde_json::Map<String, serde_json::Value> = def
            .state_fields
            .iter()
            .map(|(name, ty)| (name.clone(), json!(ty)))
            .collect();
        json!({
            "class": def.state_class,
            "fields": fields
        })
    };

    // Warn about subgraph patterns.
    if nodes
        .iter()
        .any(|n| n.description.contains("subgraph") || n.description.contains("compiled"))
    {
        warnings.push(MigrationWarning {
            item: def.graph_var.clone(),
            message: "Subgraph composition detected".into(),
            suggestion: "Nexus OS workflows are flat — nested graphs should be converted to separate workflows".into(),
        });
    }

    Ok(ConvertedWorkflow {
        original_name: def.graph_var.clone(),
        nexus_workflow_id: workflow_id,
        nodes,
        edges,
        entry_point,
        state_schema,
    })
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_GRAPH: &str = r#"
from langgraph.graph import StateGraph, END

class AgentState(TypedDict):
    messages: list[str]
    next_step: str

def agent_node(state):
    return {"messages": state["messages"] + ["hello"]}

def tool_executor(state):
    return {"messages": state["messages"] + ["tool result"]}

graph = StateGraph(AgentState)
graph.add_node("agent", agent_node)
graph.add_node("tools", tool_executor)
graph.add_edge("agent", "tools")
graph.add_edge("tools", END)
graph.set_entry_point("agent")
app = graph.compile()
"#;

    const CONDITIONAL_GRAPH: &str = r#"
from langgraph.graph import StateGraph, END

class State(TypedDict):
    messages: list[str]
    should_continue: bool

workflow = StateGraph(State)
workflow.add_node("agent", call_model)
workflow.add_node("tools", call_tools)
workflow.add_node("human", human_review)
workflow.set_entry_point("agent")
workflow.add_conditional_edges("agent", should_continue, {"continue": "tools", "end": END})
workflow.add_edge("tools", "agent")
"#;

    #[test]
    fn test_detect_state_graph() {
        let defs = LangGraphParser::parse_python(SIMPLE_GRAPH).unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].graph_var, "graph");
        assert_eq!(defs[0].state_class, "AgentState");
    }

    #[test]
    fn test_detect_nodes() {
        let defs = LangGraphParser::parse_python(SIMPLE_GRAPH).unwrap();
        assert_eq!(defs[0].nodes.len(), 2);
        assert_eq!(defs[0].nodes[0], ("agent".into(), "agent_node".into()));
        assert_eq!(defs[0].nodes[1], ("tools".into(), "tool_executor".into()));
    }

    #[test]
    fn test_detect_edges() {
        let defs = LangGraphParser::parse_python(SIMPLE_GRAPH).unwrap();
        assert_eq!(defs[0].edges.len(), 2);
        assert_eq!(defs[0].edges[0], ("agent".into(), "tools".into()));
        assert_eq!(defs[0].edges[1], ("tools".into(), "END".into()));
    }

    #[test]
    fn test_detect_entry_point() {
        let defs = LangGraphParser::parse_python(SIMPLE_GRAPH).unwrap();
        assert_eq!(defs[0].entry_point.as_deref(), Some("agent"));
    }

    #[test]
    fn test_detect_conditional_edges() {
        let defs = LangGraphParser::parse_python(CONDITIONAL_GRAPH).unwrap();
        assert_eq!(defs[0].conditional_edges.len(), 1);
        let cond = &defs[0].conditional_edges[0];
        assert_eq!(cond.source, "agent");
        assert_eq!(cond.function_name, "should_continue");
        assert_eq!(cond.routes.len(), 2);
    }

    #[test]
    fn test_detect_state_fields() {
        let defs = LangGraphParser::parse_python(SIMPLE_GRAPH).unwrap();
        let fields = &defs[0].state_fields;
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|(n, _)| n == "messages"));
        assert!(fields.iter().any(|(n, _)| n == "next_step"));
    }

    #[test]
    fn test_convert_simple_workflow() {
        let result = LangGraphParser::migrate(SIMPLE_GRAPH).unwrap();
        assert_eq!(result.summary.workflows_converted, 1);
        let wf = &result.workflows_converted[0];
        assert_eq!(wf.entry_point, "agent");
        // 2 real nodes + __start__ + __end__
        assert_eq!(wf.nodes.len(), 4);
    }

    #[test]
    fn test_convert_conditional_workflow() {
        let result = LangGraphParser::migrate(CONDITIONAL_GRAPH).unwrap();
        assert_eq!(result.summary.workflows_converted, 1);
        let wf = &result.workflows_converted[0];
        // Should have conditional edges converted.
        assert!(wf.edges.iter().any(|e| e.condition.is_some()));
    }

    #[test]
    fn test_end_node_normalized() {
        let result = LangGraphParser::migrate(SIMPLE_GRAPH).unwrap();
        let wf = &result.workflows_converted[0];
        assert!(wf.edges.iter().any(|e| e.to_node == "__end__"));
    }

    #[test]
    fn test_no_graphs_produces_warning() {
        let result = LangGraphParser::migrate("# just a comment\nprint('hello')").unwrap();
        assert_eq!(result.summary.workflows_converted, 0);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("No StateGraph")));
    }

    #[test]
    fn test_multiple_graphs_in_one_file() {
        let python = r#"
graph1 = StateGraph(State1)
graph1.add_node("a", func_a)
graph1.set_entry_point("a")

graph2 = StateGraph(State2)
graph2.add_node("b", func_b)
graph2.set_entry_point("b")
"#;
        let defs = LangGraphParser::parse_python(python).unwrap();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].graph_var, "graph1");
        assert_eq!(defs[1].graph_var, "graph2");
    }

    #[test]
    fn test_classify_tool_node() {
        assert_eq!(classify_node("tool_executor"), WorkflowNodeType::ToolCall);
        assert_eq!(classify_node("execute_actions"), WorkflowNodeType::ToolCall);
    }

    #[test]
    fn test_classify_human_node() {
        assert_eq!(classify_node("human_review"), WorkflowNodeType::HumanReview);
        assert_eq!(classify_node("approve_step"), WorkflowNodeType::HumanReview);
    }

    #[test]
    fn test_classify_agent_node() {
        assert_eq!(
            classify_node("call_model"),
            WorkflowNodeType::AgentExecution
        );
        assert_eq!(classify_node("process"), WorkflowNodeType::AgentExecution);
    }

    #[test]
    fn test_empty_graph_error() {
        let python = r#"
graph = StateGraph(State)
"#;
        let result = LangGraphParser::migrate(python).unwrap();
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].message.contains("no nodes or edges"));
    }

    #[test]
    fn test_state_schema_in_output() {
        let result = LangGraphParser::migrate(SIMPLE_GRAPH).unwrap();
        let wf = &result.workflows_converted[0];
        assert_eq!(wf.state_schema["class"], "AgentState");
        let fields = wf.state_schema["fields"].as_object().unwrap();
        assert!(fields.contains_key("messages"));
    }
}
