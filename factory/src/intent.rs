use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::LlmProvider;
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    ContentPosting,
    Research,
    Monitoring,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedIntent {
    pub task_type: TaskType,
    pub platforms: Vec<String>,
    pub schedule: String,
    pub content_topic: String,
    pub raw_request: String,
}

#[derive(Debug, Deserialize)]
struct LlmIntentOutput {
    task_type: Option<String>,
    platforms: Option<Vec<String>>,
    schedule: Option<String>,
    content_topic: Option<String>,
}

pub struct IntentParser<P: LlmProvider> {
    gateway: GovernedLlmGateway<P>,
    context: AgentRuntimeContext,
    model_name: String,
    max_tokens: u32,
}

impl<P: LlmProvider> IntentParser<P> {
    pub fn new(provider: P, model_name: &str, fuel_budget: u64) -> Self {
        let capabilities = ["llm.query".to_string()]
            .into_iter()
            .collect::<HashSet<_>>();
        Self {
            gateway: GovernedLlmGateway::new(provider),
            context: AgentRuntimeContext {
                agent_id: Uuid::new_v4(),
                capabilities,
                fuel_remaining: fuel_budget,
            },
            model_name: model_name.to_string(),
            max_tokens: 180,
        }
    }

    pub fn parse(&mut self, request: &str) -> Result<ParsedIntent, AgentError> {
        let prompt = format!(
            "Parse user intent into JSON keys task_type, platforms, schedule, content_topic. Request: {request}"
        );

        let response = self.gateway.query(
            &mut self.context,
            prompt.as_str(),
            self.max_tokens,
            self.model_name.as_str(),
        )?;

        if let Ok(output) = serde_json::from_str::<LlmIntentOutput>(response.output_text.as_str()) {
            return Ok(intent_from_llm_output(request, output));
        }

        Ok(parse_with_rules(request))
    }

    pub fn audit_oracle_count(&self) -> usize {
        self.gateway.oracle_events().len()
    }
}

fn intent_from_llm_output(request: &str, output: LlmIntentOutput) -> ParsedIntent {
    let task_type = match output
        .task_type
        .unwrap_or_else(|| "unknown".to_string())
        .to_lowercase()
        .as_str()
    {
        "contentposting" | "content_posting" | "content-posting" => TaskType::ContentPosting,
        "research" => TaskType::Research,
        "monitoring" => TaskType::Monitoring,
        _ => infer_task_type(request),
    };

    let platforms =
        normalize_platforms(output.platforms.unwrap_or_else(|| infer_platforms(request)));
    let schedule = output
        .schedule
        .unwrap_or_else(|| infer_schedule(request))
        .trim()
        .to_lowercase();
    let content_topic = output
        .content_topic
        .unwrap_or_else(|| infer_topic(request))
        .trim()
        .to_string();

    ParsedIntent {
        task_type,
        platforms,
        schedule,
        content_topic,
        raw_request: request.to_string(),
    }
}

fn parse_with_rules(request: &str) -> ParsedIntent {
    ParsedIntent {
        task_type: infer_task_type(request),
        platforms: normalize_platforms(infer_platforms(request)),
        schedule: infer_schedule(request),
        content_topic: infer_topic(request),
        raw_request: request.to_string(),
    }
}

fn infer_task_type(request: &str) -> TaskType {
    let lower = request.to_lowercase();

    if lower.contains("post") || lower.contains("publish") {
        TaskType::ContentPosting
    } else if lower.contains("research") {
        TaskType::Research
    } else if lower.contains("monitor") || lower.contains("watch") {
        TaskType::Monitoring
    } else {
        TaskType::Unknown
    }
}

fn infer_platforms(request: &str) -> Vec<String> {
    let lower = request.to_lowercase();
    let mut platforms = Vec::new();

    if lower.contains("twitter") || lower.contains("x ") || lower.ends_with(" x") {
        platforms.push("twitter".to_string());
    }
    if lower.contains("instagram") {
        platforms.push("instagram".to_string());
    }
    if lower.contains("facebook") {
        platforms.push("facebook".to_string());
    }

    if platforms.is_empty() {
        platforms.push("generic".to_string());
    }

    platforms
}

fn normalize_platforms(platforms: Vec<String>) -> Vec<String> {
    let mut normalized = platforms
        .into_iter()
        .map(|platform| platform.trim().to_lowercase())
        .filter(|platform| !platform.is_empty())
        .map(|platform| match platform.as_str() {
            "x" => "twitter".to_string(),
            _ => platform,
        })
        .collect::<Vec<_>>();

    normalized.sort();
    normalized.dedup();
    normalized
}

fn infer_schedule(request: &str) -> String {
    let lower = request.to_lowercase();

    if lower.contains("daily") {
        return "daily".to_string();
    }
    if lower.contains("every morning at 9am") || lower.contains("9am") {
        return "every morning at 9am".to_string();
    }
    if lower.contains("every morning") {
        return "every morning".to_string();
    }

    "unspecified".to_string()
}

fn infer_topic(request: &str) -> String {
    let lower = request.to_lowercase();
    let marker = "about ";

    if let Some(start) = lower.find(marker) {
        let suffix = &lower[(start + marker.len())..];
        let mut topic = suffix
            .split(" on ")
            .next()
            .unwrap_or_default()
            .split(" every ")
            .next()
            .unwrap_or_default()
            .split(" daily")
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();

        if !topic.is_empty() {
            return std::mem::take(&mut topic);
        }
    }

    "general".to_string()
}

#[cfg(test)]
mod tests {
    use super::{IntentParser, ParsedIntent, TaskType};
    use nexus_connectors_llm::providers::{LlmProvider, LlmResponse};
    use nexus_kernel::errors::AgentError;

    struct MockIntentProvider;

    impl LlmProvider for MockIntentProvider {
        fn query(
            &self,
            _prompt: &str,
            max_tokens: u32,
            model: &str,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                output_text: r#"{
                    "task_type": "ContentPosting",
                    "platforms": ["twitter"],
                    "schedule": "daily",
                    "content_topic": "ai"
                }"#
                .to_string(),
                token_count: max_tokens.min(40),
                model_name: model.to_string(),
                tool_calls: Vec::new(),
            })
        }

        fn name(&self) -> &str {
            "mock-intent"
        }

        fn cost_per_token(&self) -> f64 {
            0.0
        }
    }

    #[test]
    fn test_intent_parsing() {
        let mut parser = IntentParser::new(MockIntentProvider, "mock-model", 500);
        let parsed = parser.parse("Post about AI on Twitter daily");
        assert!(parsed.is_ok());

        if let Ok(ParsedIntent {
            task_type,
            platforms,
            schedule,
            ..
        }) = parsed
        {
            assert_eq!(task_type, TaskType::ContentPosting);
            assert_eq!(platforms, vec!["twitter".to_string()]);
            assert_eq!(schedule, "daily".to_string());
        }

        assert_eq!(parser.audit_oracle_count(), 1);
    }
}
