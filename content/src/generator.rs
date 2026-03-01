use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::LlmProvider;
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SocialPlatform {
    X,
    Instagram,
    Facebook,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformContent {
    pub platform: SocialPlatform,
    pub text: String,
    pub hashtags: Vec<String>,
    pub thread: Option<Vec<String>>,
    pub image_prompt: Option<String>,
    pub link_preview: Option<String>,
}

pub struct ContentGenerator<P: LlmProvider> {
    gateway: GovernedLlmGateway<P>,
    llm_context: AgentRuntimeContext,
    model_name: String,
}

impl<P: LlmProvider> ContentGenerator<P> {
    pub fn new(provider: P, model_name: &str, llm_fuel_budget: u64) -> Self {
        let capabilities = ["llm.query".to_string()].into_iter().collect::<HashSet<_>>();
        Self {
            gateway: GovernedLlmGateway::new(provider),
            llm_context: AgentRuntimeContext {
                agent_id: Uuid::new_v4(),
                capabilities,
                fuel_remaining: llm_fuel_budget,
            },
            model_name: model_name.to_string(),
        }
    }

    pub fn generate_post(
        &mut self,
        platform: SocialPlatform,
        topic: &str,
        style: &str,
    ) -> Result<PlatformContent, AgentError> {
        let prompt = format!(
            "Generate {platform:?} social copy about '{topic}' in '{style}' style. Return concise text only."
        );
        let response = self
            .gateway
            .query(&mut self.llm_context, prompt.as_str(), 120, self.model_name.as_str())?;

        let base = response.output_text.split_whitespace().collect::<Vec<_>>().join(" ");
        let hashtags = hashtags_from_topic(topic);

        match platform {
            SocialPlatform::X => {
                let hashtag_suffix = if hashtags.is_empty() {
                    "#nexus".to_string()
                } else {
                    hashtags.join(" ")
                };

                let mut text = format!("{} {}", base, hashtag_suffix).trim().to_string();
                if text.is_empty() {
                    text = format!("{} #nexus", topic);
                }

                let capped = cap_text(&text, 280);
                Ok(PlatformContent {
                    platform,
                    text: capped,
                    hashtags: if hashtags.is_empty() {
                        vec!["#nexus".to_string()]
                    } else {
                        hashtags
                    },
                    thread: None,
                    image_prompt: None,
                    link_preview: None,
                })
            }
            SocialPlatform::Instagram => {
                let caption = format!("{}\n\n{}", base, hashtags.join(" ")).trim().to_string();
                Ok(PlatformContent {
                    platform,
                    text: caption,
                    hashtags,
                    thread: None,
                    image_prompt: Some(format!("Vibrant visual for topic: {topic}")),
                    link_preview: None,
                })
            }
            SocialPlatform::Facebook => {
                let long = format!(
                    "{}\n\nKey takeaway: {}\nLearn more at https://example.com/{}",
                    base,
                    topic,
                    topic.replace(' ', "-").to_lowercase()
                );
                Ok(PlatformContent {
                    platform,
                    text: long,
                    hashtags,
                    thread: None,
                    image_prompt: None,
                    link_preview: Some(format!("https://example.com/{}", topic.replace(' ', "-").to_lowercase())),
                })
            }
        }
    }
}

fn hashtags_from_topic(topic: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for word in topic.split_whitespace() {
        let cleaned = word
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .collect::<String>()
            .to_lowercase();
        if cleaned.len() >= 3 {
            tags.push(format!("#{cleaned}"));
        }
    }
    if tags.is_empty() {
        tags.push("#nexus".to_string());
    }
    tags
}

fn cap_text(input: &str, max_chars: usize) -> String {
    let chars = input.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return input.to_string();
    }
    chars.into_iter().take(max_chars).collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::{ContentGenerator, SocialPlatform};
    use nexus_connectors_llm::providers::{LlmProvider, LlmResponse};
    use nexus_kernel::errors::AgentError;

    struct MockProvider;

    impl LlmProvider for MockProvider {
        fn query(
            &self,
            _prompt: &str,
            max_tokens: u32,
            model: &str,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                output_text: "Rust gives you fearless concurrency and performance at scale.".to_string(),
                token_count: max_tokens.min(40),
                model_name: model.to_string(),
                tool_calls: Vec::new(),
            })
        }

        fn name(&self) -> &str {
            "mock"
        }

        fn cost_per_token(&self) -> f64 {
            0.0
        }
    }

    #[test]
    fn test_generate_twitter_post() {
        let mut generator = ContentGenerator::new(MockProvider, "mock-model", 500);
        let content = generator.generate_post(SocialPlatform::X, "Rust programming", "concise");
        assert!(content.is_ok());

        if let Ok(content) = content {
            assert!(content.text.len() <= 280);
            assert!(content.hashtags.iter().any(|tag| tag.starts_with('#')));
        }
    }
}
