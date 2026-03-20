use crate::navigator::SocialPlatform;
use nexus_connectors_llm::gateway::{
    select_provider, AgentRuntimeContext, GovernedLlmGateway, ProviderSelectionConfig,
};
use nexus_connectors_llm::providers::LlmProvider;
use nexus_sdk::audit::{AuditEvent, AuditTrail, EventType};
use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftPost {
    pub platform: SocialPlatform,
    pub text: String,
    pub hashtags: Vec<String>,
    pub media_urls: Vec<String>,
    pub scheduled_time: Option<u64>,
    pub variants: Vec<String>,
}

pub struct ContentComposer {
    gateway: GovernedLlmGateway<Box<dyn LlmProvider>>,
    runtime: AgentRuntimeContext,
    audit_trail: AuditTrail,
}

impl Default for ContentComposer {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentComposer {
    pub fn new() -> Self {
        let config = ProviderSelectionConfig::from_env();
        let provider: Box<dyn LlmProvider> = select_provider(&config).unwrap_or_else(|_| {
            Box::new(nexus_connectors_llm::providers::OllamaProvider::from_env())
        });
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

    pub fn compose(
        &mut self,
        topic: &str,
        platform: SocialPlatform,
        style: &str,
    ) -> Result<DraftPost, AgentError> {
        let prompt = format!(
            "Compose 3 concise variants for platform={} style={} topic={}",
            platform.as_label(),
            style,
            topic
        );
        let llm_response = self
            .gateway
            .query(&mut self.runtime, prompt.as_str(), 220, "mock-1")?;

        let hashtags = derive_hashtags(topic);
        let mut variants =
            build_variants(topic, style, platform, llm_response.output_text.as_str());
        variants = variants
            .into_iter()
            .map(|value| format_for_platform(value, platform, hashtags.as_slice()))
            .collect();

        let scheduled_time = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs() + 3600)
                .unwrap_or(0),
        );

        let draft = DraftPost {
            platform,
            text: variants.first().cloned().unwrap_or_else(|| {
                format_for_platform(topic.to_string(), platform, hashtags.as_slice())
            }),
            hashtags,
            media_urls: Vec::new(),
            scheduled_time,
            variants,
        };

        if let Err(e) = self.audit_trail.append_event(
            self.runtime.agent_id,
            EventType::LlmCall,
            json!({
                "step": "compose",
                "platform": platform.as_label(),
                "topic": topic,
                "style": style,
                "variants": draft.variants.len(),
                "text_length": draft.text.chars().count(),
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }

        Ok(draft)
    }

    pub fn audit_events(&self) -> &[AuditEvent] {
        self.audit_trail.events()
    }
}

pub fn compose(
    topic: &str,
    platform: SocialPlatform,
    style: &str,
) -> Result<DraftPost, AgentError> {
    let mut composer = ContentComposer::new();
    composer.compose(topic, platform, style)
}

fn derive_hashtags(topic: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for word in topic.split_whitespace() {
        let alnum = word
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .collect::<String>();
        if alnum.len() < 3 {
            continue;
        }
        tags.push(format!("#{}", alnum.to_ascii_lowercase()));
    }
    tags.sort();
    tags.dedup();
    tags.into_iter().take(5).collect()
}

fn build_variants(
    topic: &str,
    style: &str,
    platform: SocialPlatform,
    llm_hint: &str,
) -> Vec<String> {
    let base = if llm_hint.trim().is_empty() {
        topic.to_string()
    } else {
        llm_hint.lines().next().unwrap_or(topic).to_string()
    };

    vec![
        format!(
            "{} | {} perspective on {}",
            base.trim(),
            style,
            platform.as_label()
        ),
        format!(
            "{}: key takeaway on {} ({})",
            style,
            topic,
            platform.as_label()
        ),
        format!(
            "{} update: {}",
            platform.as_label().to_ascii_uppercase(),
            topic
        ),
    ]
}

fn format_for_platform(text: String, platform: SocialPlatform, hashtags: &[String]) -> String {
    match platform {
        SocialPlatform::X => {
            let mut content = text;
            if !hashtags.is_empty() {
                content.push(' ');
                content.push_str(hashtags.join(" ").as_str());
            }
            truncate_chars(content, 280)
        }
        SocialPlatform::Instagram => {
            let mut content = format!(
                "{}\n\nImage description: vivid scene matching the caption.",
                text
            );
            if !hashtags.is_empty() {
                content.push('\n');
                content.push_str(hashtags.join(" ").as_str());
            }
            content
        }
        SocialPlatform::Facebook => {
            format!("{}\n\nRead more: add context link if needed.", text)
        }
        SocialPlatform::Reddit => {
            let title = truncate_chars(text.clone(), 120);
            let body = format!(
                "Title: {}\n\nBody: {}\n\nSubreddit: r/technology",
                title, text
            );
            truncate_chars(body, 10_000)
        }
        SocialPlatform::LinkedIn => {
            let content = format!(
                "{}\n\nProfessional insight: practical lessons and clear next steps.",
                text
            );
            truncate_chars(content, 3000)
        }
        SocialPlatform::TikTok | SocialPlatform::YouTube => {
            let mut content = format!("{}\n\nCall to action: follow for more.", text);
            if !hashtags.is_empty() {
                content.push('\n');
                content.push_str(hashtags.join(" ").as_str());
            }
            content
        }
    }
}

fn truncate_chars(input: String, limit: usize) -> String {
    let mut output = String::new();
    for (index, ch) in input.chars().enumerate() {
        if index >= limit {
            break;
        }
        output.push(ch);
    }
    output
}
