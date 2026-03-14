use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::LlmProvider;
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    ContentPosting,
    FileBackup,
    Research,
    Monitoring,
    SelfImprove,
    WebBuild,
    CodeGen,
    DesignGen,
    FixProject,
    CloneSite,
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
            r#"Parse the user request into JSON with keys: task_type, platforms, schedule, content_topic.

task_type must be one of:
- "content_posting" for social media posts, publishing content
- "file_backup" for backing up or archiving files
- "research" for research tasks
- "monitoring" for monitoring or watching services
- "self_improve" for self-improvement, prompt optimization
- "web_build" for requests about websites, landing pages, portfolios, 3D scenes, HTML/CSS
- "code_gen" for requests about apps, APIs, auth, backends, databases, Stripe
- "design_gen" for requests about design systems, component libraries, themes, branding
- "fix_project" for requests about fixing bugs, making tests pass, debugging
- "clone_site" for requests that include a URL and words like "clone", "copy", "recreate"
- "unknown" if none of the above match

platforms is an array of relevant platforms/technologies (e.g. ["twitter"], ["html", "react", "threejs"]).
schedule is a cron or human-readable schedule, or "unspecified".
content_topic is the subject matter.

Respond with ONLY valid JSON. Request: {request}"#
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
        "filebackup" | "file_backup" | "file-backup" => TaskType::FileBackup,
        "research" => TaskType::Research,
        "monitoring" => TaskType::Monitoring,
        "selfimprove" | "self_improve" | "self-improve" => TaskType::SelfImprove,
        "webbuild" | "web_build" | "web-build" => TaskType::WebBuild,
        "codegen" | "code_gen" | "code-gen" => TaskType::CodeGen,
        "designgen" | "design_gen" | "design-gen" => TaskType::DesignGen,
        "fixproject" | "fix_project" | "fix-project" => TaskType::FixProject,
        "clonesite" | "clone_site" | "clone-site" => TaskType::CloneSite,
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

    // Clone site: URL-like pattern + clone/copy/recreate keywords (check first — most specific)
    let has_url = lower.contains("http://") || lower.contains("https://") || lower.contains(".com");
    let has_clone_word =
        lower.contains("clone") || lower.contains("copy") || lower.contains("recreate");
    if has_url && has_clone_word {
        return TaskType::CloneSite;
    }

    // Fix project: debugging, fixing bugs, making tests pass
    if lower.contains("fix")
        || lower.contains("debug")
        || lower.contains("make tests pass")
        || lower.contains("bug")
    {
        return TaskType::FixProject;
    }

    if lower.contains("back up")
        || lower.contains("backup")
        || (lower.contains("archive") && lower.contains("file"))
    {
        TaskType::FileBackup
    } else if lower.contains("post") || lower.contains("publish") {
        TaskType::ContentPosting
    } else if lower.contains("research") {
        TaskType::Research
    } else if lower.contains("monitor") || lower.contains("watch") {
        TaskType::Monitoring
    } else if lower.contains("self-improve")
        || lower.contains("self improve")
        || lower.contains("optimize prompts")
        || lower.contains("learn from outcomes")
    {
        TaskType::SelfImprove
    } else if lower.contains("website")
        || lower.contains("landing page")
        || lower.contains("portfolio")
        || lower.contains("3d scene")
        || lower.contains("html")
        || lower.contains("css")
    {
        TaskType::WebBuild
    } else if lower.contains("app")
        || lower.contains("api")
        || lower.contains("auth")
        || lower.contains("backend")
        || lower.contains("database")
        || lower.contains("stripe")
    {
        TaskType::CodeGen
    } else if lower.contains("design system")
        || lower.contains("component librar")
        || lower.contains("theme")
        || lower.contains("branding")
        || lower.contains("design token")
    {
        TaskType::DesignGen
    } else {
        TaskType::Unknown
    }
}

fn infer_platforms(request: &str) -> Vec<String> {
    let lower = request.to_lowercase();
    let mut platforms = Vec::new();

    // Social platforms
    if lower.contains("twitter") || lower.contains("x ") || lower.ends_with(" x") {
        platforms.push("twitter".to_string());
    }
    if lower.contains("instagram") {
        platforms.push("instagram".to_string());
    }
    if lower.contains("facebook") {
        platforms.push("facebook".to_string());
    }

    // Web/code technologies
    if lower.contains("html") {
        platforms.push("html".to_string());
    }
    if lower.contains("react") {
        platforms.push("react".to_string());
    }
    if lower.contains("threejs") || lower.contains("three.js") || lower.contains("3d") {
        platforms.push("threejs".to_string());
    }
    if lower.contains("css") || lower.contains("tailwind") {
        platforms.push("css".to_string());
    }
    if lower.contains("stripe") {
        platforms.push("stripe".to_string());
    }
    if lower.contains("next.js") || lower.contains("nextjs") {
        platforms.push("nextjs".to_string());
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

    if lower.contains("every night") || lower.contains("nightly") {
        return "0 0 * * *".to_string();
    }
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

    if lower.contains("back up ") || lower.contains("backup ") {
        let candidate = lower
            .replace("back up ", "")
            .replace("backup ", "")
            .split(" every ")
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        if !candidate.is_empty() {
            return candidate;
        }
    }

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
    fn test_web_build_intent() {
        let parsed = super::parse_with_rules("build me a portfolio website");
        assert_eq!(parsed.task_type, TaskType::WebBuild);
    }

    #[test]
    fn test_code_gen_intent() {
        let parsed = super::parse_with_rules("add authentication to my app");
        assert_eq!(parsed.task_type, TaskType::CodeGen);
    }

    #[test]
    fn test_design_gen_intent() {
        let parsed = super::parse_with_rules("create a design system with dark mode");
        assert_eq!(parsed.task_type, TaskType::DesignGen);
    }

    #[test]
    fn test_fix_project_intent() {
        let parsed = super::parse_with_rules("fix the bugs in ./my-project");
        assert_eq!(parsed.task_type, TaskType::FixProject);
    }

    #[test]
    fn test_clone_site_intent() {
        let parsed =
            super::parse_with_rules("clone https://example.com and make it modern");
        assert_eq!(parsed.task_type, TaskType::CloneSite);
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
