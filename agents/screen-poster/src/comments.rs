use crate::stealth::{gaussian_action_delays_ms_seeded, StealthProfile};
use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::{LlmProvider, MockProvider};
use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sentiment {
    Positive,
    Neutral,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub author: String,
    pub text: String,
    pub timestamp: String,
    pub sentiment: Sentiment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyDraft {
    pub comment_author: String,
    pub text: String,
    pub scheduled_delay_ms: u64,
}

pub trait CommentSource {
    fn scan_comments(&mut self, post_url: &str) -> Result<Vec<Comment>, AgentError>;
}

#[derive(Debug, Default)]
pub struct MockCommentSource;

impl CommentSource for MockCommentSource {
    fn scan_comments(&mut self, _post_url: &str) -> Result<Vec<Comment>, AgentError> {
        Ok(vec![
            Comment {
                author: "alex".to_string(),
                text: "Great breakdown, can you share implementation details?".to_string(),
                timestamp: "2026-03-03T10:00:00Z".to_string(),
                sentiment: Sentiment::Positive,
            },
            Comment {
                author: "sam".to_string(),
                text: "I tried this and got stuck at setup.".to_string(),
                timestamp: "2026-03-03T10:05:00Z".to_string(),
                sentiment: Sentiment::Neutral,
            },
        ])
    }
}

pub struct CommentInteractor {
    gateway: GovernedLlmGateway<Box<dyn LlmProvider>>,
    runtime: AgentRuntimeContext,
    profile: StealthProfile,
}

impl Default for CommentInteractor {
    fn default() -> Self {
        Self::new()
    }
}

impl CommentInteractor {
    pub fn new() -> Self {
        let provider: Box<dyn LlmProvider> = Box::new(MockProvider::new());
        let gateway = GovernedLlmGateway::new(provider);
        let capabilities = ["llm.query".to_string()]
            .into_iter()
            .collect::<HashSet<_>>();
        let runtime = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities,
            fuel_remaining: 2_500,
        };

        Self {
            gateway,
            runtime,
            profile: StealthProfile::default(),
        }
    }

    pub fn scan_comments(
        &mut self,
        source: &mut dyn CommentSource,
        post_url: &str,
    ) -> Result<Vec<Comment>, AgentError> {
        let comments = source.scan_comments(post_url)?;
        Ok(select_relevant_comments(comments, 5))
    }

    pub fn generate_reply(
        &mut self,
        comment: &Comment,
        context: &str,
    ) -> Result<ReplyDraft, AgentError> {
        let prompt = format!(
            "Write one concise reply in a human tone. Comment='{}' Context='{}'",
            comment.text, context
        );
        let response = self
            .gateway
            .query(&mut self.runtime, prompt.as_str(), 140, "mock-1")?;
        let draft_text = if response.output_text.trim().is_empty() {
            fallback_reply(comment, context)
        } else {
            format_reply(response.output_text.as_str(), comment)
        };

        let delay = gaussian_action_delays_ms_seeded(1, &self.profile, 1337)
            .first()
            .copied()
            .unwrap_or(1_000);

        Ok(ReplyDraft {
            comment_author: comment.author.clone(),
            text: draft_text,
            scheduled_delay_ms: delay,
        })
    }
}

pub fn scan_comments(post_url: &str) -> Result<Vec<Comment>, AgentError> {
    let mut interactor = CommentInteractor::new();
    let mut source = MockCommentSource;
    interactor.scan_comments(&mut source, post_url)
}

pub fn generate_reply(comment: &Comment, context: &str) -> Result<ReplyDraft, AgentError> {
    let mut interactor = CommentInteractor::new();
    interactor.generate_reply(comment, context)
}

fn select_relevant_comments(comments: Vec<Comment>, max_count: usize) -> Vec<Comment> {
    let mut scored = comments
        .into_iter()
        .map(|comment| {
            let score = match comment.sentiment {
                Sentiment::Positive => 3,
                Sentiment::Neutral => 2,
                Sentiment::Negative => 1,
            } + if comment.text.contains('?') { 2 } else { 0 };
            (score, comment)
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| right.0.cmp(&left.0));
    scored
        .into_iter()
        .take(max_count.clamp(3, 5))
        .map(|(_, comment)| comment)
        .collect()
}

fn fallback_reply(comment: &Comment, context: &str) -> String {
    format!(
        "Thanks {}, great point. {} We will share a concrete follow-up.",
        comment.author,
        truncate(context, 100)
    )
}

fn format_reply(raw: &str, comment: &Comment) -> String {
    let cleaned = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("@{} {}", comment.author, truncate(cleaned.as_str(), 240))
}

fn truncate(input: &str, limit: usize) -> String {
    input.chars().take(limit).collect::<String>()
}
