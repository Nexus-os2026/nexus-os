use crate::context::CodeContext;
use crate::scanner::{Language, ProjectMap};
use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::{LlmProvider, MockProvider};
use nexus_sdk::audit::{AuditEvent, AuditTrail, EventType};
use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileChange {
    Create(String, String),
    Modify(String, String, String),
    Delete(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamingConvention {
    SnakeCase,
    CamelCase,
    PascalCase,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleProfile {
    pub indent_width: usize,
    pub uses_tabs: bool,
    pub naming_convention: NamingConvention,
    pub comment_style: String,
    pub import_organization: String,
    pub error_handling_pattern: String,
}

pub struct CodeWriter {
    gateway: GovernedLlmGateway<Box<dyn LlmProvider>>,
    runtime: AgentRuntimeContext,
    audit_trail: AuditTrail,
}

impl Default for CodeWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeWriter {
    pub fn new() -> Self {
        let provider: Box<dyn LlmProvider> = Box::new(MockProvider::new());
        let gateway = GovernedLlmGateway::new(provider);
        let capabilities = ["llm.query".to_string()]
            .into_iter()
            .collect::<HashSet<_>>();
        let runtime = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities,
            fuel_remaining: 5_000,
        };

        Self {
            gateway,
            runtime,
            audit_trail: AuditTrail::new(),
        }
    }

    pub fn write_code(
        &mut self,
        context: &CodeContext,
        task: &str,
    ) -> Result<Vec<FileChange>, AgentError> {
        let prompt = format!(
            "Task: {task}\nContext files: {}\nReturn concise strategy.",
            context.files.len()
        );
        let _ = self
            .gateway
            .query(&mut self.runtime, prompt.as_str(), 64, "mock-1")?;

        let style = infer_style_from_context(context);
        let mut changes = Vec::new();
        let lower_task = task.to_ascii_lowercase();
        if lower_task.contains("connector") {
            let indent = if style.uses_tabs {
                "\t".to_string()
            } else {
                " ".repeat(style.indent_width.max(1))
            };
            let file_body = generated_connector_template(indent.as_str());
            changes.push(FileChange::Create(
                "connectors/core/src/generated_connector.rs".to_string(),
                file_body,
            ));
        }

        for change in &changes {
            self.audit_trail
                .append_event(
                    self.runtime.agent_id,
                    EventType::ToolCall,
                    json!({
                        "step": "write_code",
                        "task": task,
                        "change": format!("{change:?}"),
                    }),
                )
                .expect("audit: fail-closed");
        }

        Ok(changes)
    }

    pub fn audit_events(&self) -> &[AuditEvent] {
        self.audit_trail.events()
    }
}

pub fn write_code(context: &CodeContext, task: &str) -> Result<Vec<FileChange>, AgentError> {
    let mut writer = CodeWriter::new();
    writer.write_code(context, task)
}

pub fn detect_style(project_map: &ProjectMap) -> Result<StyleProfile, AgentError> {
    let root = PathBuf::from(project_map.root_path.as_str());
    let mut tab_indent_lines = 0_usize;
    let mut space_indent_counts = Vec::new();
    let mut snake = 0_usize;
    let mut camel = 0_usize;
    let mut pascal = 0_usize;
    let mut slash_comments = 0_usize;
    let mut hash_comments = 0_usize;
    let mut grouped_import_blocks = 0_usize;
    let mut question_mark_errors = 0_usize;
    let mut map_err_errors = 0_usize;

    for entry in &project_map.file_tree {
        if entry.language != Language::Rust {
            continue;
        }
        let full_path = root.join(entry.path.as_str());
        let Ok(content) = fs::read_to_string(full_path) else {
            continue;
        };

        let mut previous_was_use = false;
        let mut previous_was_empty = false;
        for line in content.lines() {
            let trimmed = line.trim_start();
            if line.starts_with('\t') && !trimmed.is_empty() {
                tab_indent_lines += 1;
            } else {
                let leading_spaces = line.chars().take_while(|ch| *ch == ' ').count();
                if leading_spaces > 0 && !trimmed.is_empty() {
                    space_indent_counts.push(leading_spaces);
                }
            }

            if let Some(name) = parse_rust_fn_name(trimmed) {
                if is_snake_case(name.as_str()) {
                    snake += 1;
                } else if is_camel_case(name.as_str()) {
                    camel += 1;
                } else if is_pascal_case(name.as_str()) {
                    pascal += 1;
                }
            }

            if trimmed.starts_with("//") {
                slash_comments += 1;
            } else if trimmed.starts_with("# ") {
                hash_comments += 1;
            }

            if trimmed.starts_with("use ") {
                if previous_was_use && previous_was_empty {
                    grouped_import_blocks += 1;
                }
                previous_was_use = true;
                previous_was_empty = false;
            } else if trimmed.is_empty() {
                previous_was_empty = true;
            } else {
                previous_was_use = false;
                previous_was_empty = false;
            }

            if trimmed.contains('?') {
                question_mark_errors += 1;
            }
            if trimmed.contains("map_err(") {
                map_err_errors += 1;
            }
        }
    }

    let uses_tabs = tab_indent_lines > space_indent_counts.len();
    let indent_width = if uses_tabs {
        1
    } else {
        detect_space_indent_width(space_indent_counts.as_slice()).unwrap_or(4)
    };
    let naming_convention = if snake >= camel && snake >= pascal {
        NamingConvention::SnakeCase
    } else if camel >= snake && camel >= pascal {
        NamingConvention::CamelCase
    } else if pascal >= snake && pascal >= camel {
        NamingConvention::PascalCase
    } else {
        NamingConvention::Mixed
    };
    let comment_style = if slash_comments >= hash_comments {
        "//".to_string()
    } else {
        "#".to_string()
    };
    let import_organization = if grouped_import_blocks > 0 {
        "grouped".to_string()
    } else {
        "flat".to_string()
    };
    let error_handling_pattern = if question_mark_errors >= map_err_errors {
        "result-question-mark".to_string()
    } else {
        "map-err-heavy".to_string()
    };

    Ok(StyleProfile {
        indent_width,
        uses_tabs,
        naming_convention,
        comment_style,
        import_organization,
        error_handling_pattern,
    })
}

fn generated_connector_template(indent: &str) -> String {
    format!(
        "use crate::connector::{{Connector, HealthStatus, RetryPolicy}};\n\
         use nexus_sdk::errors::AgentError;\n\n\
         #[derive(Debug, Clone, Default)]\n\
         pub struct GeneratedConnector;\n\n\
         impl Connector for GeneratedConnector {{\n\
         {indent}fn id(&self) -> &str {{\n\
         {indent}{indent}\"generated-connector\"\n\
         {indent}}}\n\n\
         {indent}fn name(&self) -> &str {{\n\
         {indent}{indent}\"Generated Connector\"\n\
         {indent}}}\n\n\
         {indent}fn required_capabilities(&self) -> Vec<String> {{\n\
         {indent}{indent}vec![\"connector.generated\".to_string()]\n\
         {indent}}}\n\n\
         {indent}fn health_check(&self) -> Result<HealthStatus, AgentError> {{\n\
         {indent}{indent}Ok(HealthStatus::Healthy)\n\
         {indent}}}\n\n\
         {indent}fn retry_policy(&self) -> RetryPolicy {{\n\
         {indent}{indent}RetryPolicy {{\n\
         {indent}{indent}{indent}max_retries: 2,\n\
         {indent}{indent}{indent}backoff_ms: 200,\n\
         {indent}{indent}{indent}backoff_multiplier: 2.0,\n\
         {indent}{indent}}}\n\
         {indent}}}\n\n\
         {indent}fn degrade_gracefully(&self) -> bool {{\n\
         {indent}{indent}true\n\
         {indent}}}\n\
         }}\n"
    )
}

fn infer_style_from_context(context: &CodeContext) -> StyleProfile {
    let mut tabs = 0_usize;
    let mut spaces = Vec::new();
    let mut slash = 0_usize;
    let mut hash = 0_usize;

    for file in &context.files {
        for line in file.content.lines() {
            let trimmed = line.trim_start();
            if line.starts_with('\t') && !trimmed.is_empty() {
                tabs += 1;
            } else {
                let leading = line.chars().take_while(|ch| *ch == ' ').count();
                if leading > 0 && !trimmed.is_empty() {
                    spaces.push(leading);
                }
            }

            if trimmed.starts_with("//") {
                slash += 1;
            } else if trimmed.starts_with("# ") {
                hash += 1;
            }
        }
    }

    let uses_tabs = tabs > spaces.len();
    let indent_width = if uses_tabs {
        1
    } else {
        detect_space_indent_width(spaces.as_slice()).unwrap_or(4)
    };
    let comment_style = if slash >= hash {
        "//".to_string()
    } else {
        "#".to_string()
    };

    StyleProfile {
        indent_width,
        uses_tabs,
        naming_convention: NamingConvention::SnakeCase,
        comment_style,
        import_organization: "grouped".to_string(),
        error_handling_pattern: "result-question-mark".to_string(),
    }
}

fn parse_rust_fn_name(line: &str) -> Option<String> {
    let prefix = "fn ";
    let start = if line.starts_with("pub fn ") {
        "pub fn "
    } else if line.starts_with(prefix) {
        prefix
    } else {
        return None;
    };
    let rest = line.strip_prefix(start)?;
    let name = rest
        .split(['(', '<', ' '])
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }
    Some(name)
}

fn detect_space_indent_width(counts: &[usize]) -> Option<usize> {
    if counts.is_empty() {
        return None;
    }

    let divisible_by_four = counts.iter().filter(|value| **value % 4 == 0).count();
    if divisible_by_four * 2 >= counts.len() {
        return Some(4);
    }

    let divisible_by_two = counts.iter().filter(|value| **value % 2 == 0).count();
    if divisible_by_two * 2 >= counts.len() {
        return Some(2);
    }

    counts.iter().copied().min()
}

fn is_snake_case(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

fn is_camel_case(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase() && !value.contains('_')
}

fn is_pascal_case(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_uppercase() && !value.contains('_')
}
