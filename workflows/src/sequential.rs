use content::compliance::{check_compliance, ComplianceDecision};
use content::generator::{PlatformContent, SocialPlatform};
use nexus_connectors_core::idempotency::IdempotencyManager;
use nexus_connectors_social::facebook::{FacebookConnector, FacebookPostResult};
use nexus_connectors_social::instagram::{InstagramConnector, InstagramPublishResult};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use nexus_research::pipeline::ResearchReport;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishOutcome {
    pub platform: SocialPlatform,
    pub post_id: String,
    pub cached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowReport {
    pub total_platforms: usize,
    pub successes: usize,
    pub failures: usize,
    pub outcomes: Vec<PublishOutcome>,
}

pub trait ContentGeneratorPort {
    fn generate(
        &mut self,
        platform: SocialPlatform,
        topic: &str,
        style: &str,
    ) -> Result<PlatformContent, AgentError>;
}

pub trait ReviewGatePort {
    fn approve(&mut self, strategy_summary: &str) -> bool;
}

pub struct SequentialWorkflow {
    pub audit_trail: AuditTrail,
    pub facebook: FacebookConnector,
    pub instagram: InstagramConnector,
    idempotency: IdempotencyManager,
    recent_x_posts: usize,
    recent_ig_posts: usize,
    recent_fb_posts: usize,
}

impl SequentialWorkflow {
    pub fn new() -> Self {
        Self {
            audit_trail: AuditTrail::new(),
            facebook: FacebookConnector::new(),
            instagram: InstagramConnector::new(),
            idempotency: IdempotencyManager::new(3_600),
            recent_x_posts: 0,
            recent_ig_posts: 0,
            recent_fb_posts: 0,
        }
    }

    pub fn execute(
        &mut self,
        generator: &mut dyn ContentGeneratorPort,
        review: &mut dyn ReviewGatePort,
        research: &ResearchReport,
        topic: &str,
        style: &str,
        platforms: &[SocialPlatform],
    ) -> WorkflowReport {
        let strategy_summary = format!(
            "Topic '{}' with {} citations and {} findings",
            research.topic,
            research.citations.len(),
            research.insights.len()
        );

        if !review.approve(strategy_summary.as_str()) {
            self.audit_trail
                .append_event(
                    uuid::Uuid::nil(),
                    EventType::UserAction,
                    json!({
                        "event": "workflow_review_rejected",
                        "topic": topic
                    }),
                )
                .expect("audit: fail-closed");
            return WorkflowReport {
                total_platforms: platforms.len(),
                successes: 0,
                failures: platforms.len(),
                outcomes: Vec::new(),
            };
        }

        let mut outcomes = Vec::new();
        let mut failures = 0;

        for platform in platforms {
            let compliance = self.check_platform_compliance(*platform);
            if let ComplianceDecision::Blocked(reason) = compliance {
                failures += 1;
                self.audit_trail
                    .append_event(
                        uuid::Uuid::nil(),
                        EventType::Error,
                        json!({
                            "event": "workflow_platform_blocked",
                            "platform": format!("{platform:?}"),
                            "reason": reason
                        }),
                    )
                    .expect("audit: fail-closed");
                continue;
            }

            let content = generator.generate(*platform, topic, style);
            let Ok(content) = content else {
                failures += 1;
                continue;
            };

            let request_id = IdempotencyManager::generate_request_id();
            let publish_result = self.publish_platform(*platform, &content, request_id.as_str());

            match publish_result {
                Ok(outcome) => {
                    outcomes.push(outcome);
                    self.bump_platform_count(*platform);
                }
                Err(_) => {
                    failures += 1;
                    continue;
                }
            }
        }

        let successes = outcomes.len();
        let report = WorkflowReport {
            total_platforms: platforms.len(),
            successes,
            failures,
            outcomes,
        };

        self.audit_trail
            .append_event(
                uuid::Uuid::nil(),
                EventType::ToolCall,
                json!({
                    "event": "workflow_completed",
                    "successes": report.successes,
                    "failures": report.failures,
                    "total": report.total_platforms
                }),
            )
            .expect("audit: fail-closed");

        report
    }

    pub fn publish_platform(
        &mut self,
        platform: SocialPlatform,
        content: &PlatformContent,
        request_id: &str,
    ) -> Result<PublishOutcome, AgentError> {
        if let Some(cached) = self.idempotency.check_duplicate(request_id) {
            return Ok(PublishOutcome {
                platform,
                post_id: cached,
                cached: true,
            });
        }

        let outcome = match platform {
            SocialPlatform::X => {
                let post_id = format!("x-{}", uuid::Uuid::new_v4());
                PublishOutcome {
                    platform,
                    post_id,
                    cached: false,
                }
            }
            SocialPlatform::Instagram => {
                let InstagramPublishResult {
                    media_id, cached, ..
                } = self.instagram.publish(content.text.as_str(), request_id)?;
                PublishOutcome {
                    platform,
                    post_id: media_id,
                    cached,
                }
            }
            SocialPlatform::Facebook => {
                let FacebookPostResult {
                    post_id, cached, ..
                } = self.facebook.post(content.text.as_str(), request_id)?;
                PublishOutcome {
                    platform,
                    post_id,
                    cached,
                }
            }
        };

        self.idempotency
            .record_completion(request_id, outcome.post_id.clone());

        Ok(outcome)
    }

    fn check_platform_compliance(&self, platform: SocialPlatform) -> ComplianceDecision {
        let recent = match platform {
            SocialPlatform::X => self.recent_x_posts,
            SocialPlatform::Instagram => self.recent_ig_posts,
            SocialPlatform::Facebook => self.recent_fb_posts,
        };
        check_compliance(platform, recent)
    }

    fn bump_platform_count(&mut self, platform: SocialPlatform) {
        match platform {
            SocialPlatform::X => self.recent_x_posts += 1,
            SocialPlatform::Instagram => self.recent_ig_posts += 1,
            SocialPlatform::Facebook => self.recent_fb_posts += 1,
        }
    }
}

impl Default for SequentialWorkflow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{ContentGeneratorPort, ReviewGatePort, SequentialWorkflow};
    use content::generator::{PlatformContent, SocialPlatform};
    use nexus_kernel::errors::AgentError;
    use nexus_research::pipeline::{Citation, ExtractedInsight, ResearchReport};

    struct MockGenerator;

    impl ContentGeneratorPort for MockGenerator {
        fn generate(
            &mut self,
            platform: SocialPlatform,
            topic: &str,
            _style: &str,
        ) -> Result<PlatformContent, AgentError> {
            let text = match platform {
                SocialPlatform::X => format!("{topic} on X #rust"),
                SocialPlatform::Instagram => format!("{topic} on IG #rust"),
                SocialPlatform::Facebook => format!("{topic} on Facebook #rust"),
            };

            Ok(PlatformContent {
                platform,
                text,
                hashtags: vec!["#rust".to_string()],
                thread: None,
                image_prompt: None,
                link_preview: None,
            })
        }
    }

    struct ApprovingReview;
    impl ReviewGatePort for ApprovingReview {
        fn approve(&mut self, _strategy_summary: &str) -> bool {
            true
        }
    }

    fn sample_report() -> ResearchReport {
        ResearchReport {
            topic: "Rust programming".to_string(),
            citations: vec![Citation {
                title: "source".to_string(),
                url: "https://example.com".to_string(),
                snippet: "snippet".to_string(),
            }],
            insights: vec![ExtractedInsight {
                source_url: "https://example.com".to_string(),
                summary: "insight".to_string(),
            }],
            read_articles: 1,
            fuel_budget: 100,
            fuel_consumed: 30,
            remaining_fuel: 70,
        }
    }

    #[test]
    fn test_sequential_workflow() {
        let mut workflow = SequentialWorkflow::new();
        let mut generator = MockGenerator;
        let mut review = ApprovingReview;

        let report = workflow.execute(
            &mut generator,
            &mut review,
            &sample_report(),
            "Rust programming",
            "educational",
            &[SocialPlatform::X, SocialPlatform::Instagram],
        );

        assert_eq!(report.successes, 2);
        assert_eq!(report.failures, 0);
    }

    #[test]
    fn test_idempotent_publish() {
        let mut workflow = SequentialWorkflow::new();
        let content = PlatformContent {
            platform: SocialPlatform::Instagram,
            text: "Rust carousel #rust".to_string(),
            hashtags: vec!["#rust".to_string()],
            thread: None,
            image_prompt: None,
            link_preview: None,
        };

        let first = workflow.publish_platform(SocialPlatform::Instagram, &content, "abc");
        assert!(first.is_ok());

        let second = workflow.publish_platform(SocialPlatform::Instagram, &content, "abc");
        assert!(second.is_ok());

        if let (Ok(first), Ok(second)) = (first, second) {
            assert_eq!(first.post_id, second.post_id);
            assert!(second.cached);
        }

        assert_eq!(workflow.instagram.published_count(), 1);
    }
}
