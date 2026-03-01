use crate::intent::{ParsedIntent, TaskType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComposableBlock {
    ResearchStep,
    GenerateContentStep,
    PublishStep,
    AnalyzeStep,
    AdaptStep,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedAgentCode {
    pub blocks: Vec<ComposableBlock>,
    pub source: String,
}

pub fn generate_agent_code(intent: &ParsedIntent) -> GeneratedAgentCode {
    let blocks = blocks_for_intent(intent);
    let source = render_source(intent, blocks.as_slice());

    GeneratedAgentCode { blocks, source }
}

pub fn passes_nex_safety_checks(code: &GeneratedAgentCode) -> bool {
    let banned_tokens = [
        "unsafe",
        "std::process::Command",
        "tokio::process",
        "fs::remove_dir_all",
    ];
    if banned_tokens
        .iter()
        .any(|token| code.source.contains(token))
    {
        return false;
    }

    let allowed_block_names = [
        "research_step",
        "generate_content_step",
        "publish_step",
        "analyze_step",
        "adapt_step",
    ];

    code.source
        .lines()
        .filter(|line| line.trim().starts_with("fn "))
        .all(|line| allowed_block_names.iter().any(|name| line.contains(name)))
}

fn blocks_for_intent(intent: &ParsedIntent) -> Vec<ComposableBlock> {
    match intent.task_type {
        TaskType::ContentPosting => vec![
            ComposableBlock::ResearchStep,
            ComposableBlock::GenerateContentStep,
            ComposableBlock::PublishStep,
            ComposableBlock::AnalyzeStep,
            ComposableBlock::AdaptStep,
        ],
        TaskType::Research => vec![
            ComposableBlock::ResearchStep,
            ComposableBlock::AnalyzeStep,
            ComposableBlock::AdaptStep,
        ],
        TaskType::Monitoring => vec![ComposableBlock::ResearchStep, ComposableBlock::AnalyzeStep],
        TaskType::Unknown => vec![ComposableBlock::ResearchStep],
    }
}

fn render_source(intent: &ParsedIntent, blocks: &[ComposableBlock]) -> String {
    let mut functions = Vec::new();

    for block in blocks {
        match block {
            ComposableBlock::ResearchStep => {
                functions.push(
                    "fn research_step() -> &'static str { \"research_complete\" }".to_string(),
                );
            }
            ComposableBlock::GenerateContentStep => {
                functions.push(
                    "fn generate_content_step() -> &'static str { \"content_generated\" }"
                        .to_string(),
                );
            }
            ComposableBlock::PublishStep => {
                functions.push("fn publish_step() -> &'static str { \"published\" }".to_string());
            }
            ComposableBlock::AnalyzeStep => {
                functions.push("fn analyze_step() -> &'static str { \"analyzed\" }".to_string());
            }
            ComposableBlock::AdaptStep => {
                functions.push("fn adapt_step() -> &'static str { \"adapted\" }".to_string());
            }
        }
    }

    let run_line = blocks
        .iter()
        .map(|block| match block {
            ComposableBlock::ResearchStep => "research_step()",
            ComposableBlock::GenerateContentStep => "generate_content_step()",
            ComposableBlock::PublishStep => "publish_step()",
            ComposableBlock::AnalyzeStep => "analyze_step()",
            ComposableBlock::AdaptStep => "adapt_step()",
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "// generated_agent for topic: {}\n{}\nfn run() -> Vec<&'static str> {{ vec![{}] }}\n",
        intent.content_topic,
        functions.join("\n"),
        run_line
    )
}

#[cfg(test)]
mod tests {
    use super::{generate_agent_code, passes_nex_safety_checks};
    use crate::intent::{ParsedIntent, TaskType};

    #[test]
    fn test_generated_code_safety() {
        let intent = ParsedIntent {
            task_type: TaskType::ContentPosting,
            platforms: vec!["twitter".to_string()],
            schedule: "daily".to_string(),
            content_topic: "rust".to_string(),
            raw_request: "post rust updates".to_string(),
        };

        let generated = generate_agent_code(&intent);
        assert!(passes_nex_safety_checks(&generated));
    }
}
