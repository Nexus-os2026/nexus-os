use nexus_connectors_web::reader::CleanContent;
use nexus_connectors_web::search::SearchResult;
use nexus_connectors_web::twitter::TweetResult;
use nexus_content::generator::{PlatformContent, SocialPlatform};
use nexus_kernel::errors::AgentError;
use social_poster_agent::{
    GenerateStep, PipelineDependencies, PublishStep, ReaderStep, SearchStep, SocialPosterAgent,
    SocialPosterConfig, SocialPosterManifest,
};

struct MockSearch;

impl SearchStep for MockSearch {
    fn search(
        &mut self,
        topic: &str,
        _max_results: usize,
    ) -> Result<Vec<SearchResult>, AgentError> {
        Ok((0..3)
            .map(|index| SearchResult {
                title: format!("{topic} article {index}"),
                url: format!("https://example.com/{index}"),
                snippet: format!("snippet-{index}"),
                relevance_score: 0.9,
            })
            .collect())
    }
}

struct MockReader;

impl ReaderStep for MockReader {
    fn read(&mut self, url: &str) -> Result<CleanContent, AgentError> {
        Ok(CleanContent {
            title: format!("title-{url}"),
            text: format!("content from {url}"),
            word_count: 3,
            source_url: url.to_string(),
            extracted_at: 0,
        })
    }
}

struct MockGenerator;

impl GenerateStep for MockGenerator {
    fn generate(
        &mut self,
        platform: SocialPlatform,
        topic: &str,
        _style: &str,
    ) -> Result<PlatformContent, AgentError> {
        Ok(PlatformContent {
            platform,
            text: format!("Generated post for {topic}"),
            hashtags: vec!["#ai".to_string(), "#technology".to_string()],
            thread: None,
            image_prompt: None,
            link_preview: None,
        })
    }
}

#[derive(Default)]
struct MockPublisher {
    calls: usize,
}

impl PublishStep for MockPublisher {
    fn publish_x(&mut self, _text: &str) -> Result<TweetResult, AgentError> {
        self.calls += 1;
        Ok(TweetResult {
            tweet_id: format!("tweet-{}", self.calls),
            posted_at: 0,
        })
    }

    fn publish_calls(&self) -> usize {
        self.calls
    }
}

#[test]
fn test_social_poster_dry_run() {
    let manifest = SocialPosterManifest {
        name: "social-poster".to_string(),
        version: "1.0.0".to_string(),
        capabilities: vec![
            "web.search".to_string(),
            "web.read".to_string(),
            "llm.query".to_string(),
            "social.x.post".to_string(),
        ],
        fuel_budget: 10_000,
        schedule: Some("0 9 * * *".to_string()),
        llm_model: Some("claude-sonnet-4-5".to_string()),
        config: SocialPosterConfig {
            topic: "AI and technology".to_string(),
            platforms: vec!["x".to_string()],
            style: "professional".to_string(),
            posts_per_day: 2,
        },
    };

    let dependencies = PipelineDependencies {
        search: Box::new(MockSearch),
        reader: Box::new(MockReader),
        generator: Box::new(MockGenerator),
        publisher: Box::new(MockPublisher::default()),
    };
    let mut agent = SocialPosterAgent::with_dependencies(manifest, true, dependencies);

    let report = agent.run().expect("dry-run pipeline should succeed");
    assert!(report.dry_run);
    assert_eq!(report.published_post_ids.len(), 0);
    assert_eq!(report.publish_calls, 0);
    assert!(!report.generated_posts.is_empty());

    let steps = report
        .audit_events
        .iter()
        .filter_map(|event| event.payload.get("step").and_then(|value| value.as_str()))
        .collect::<Vec<_>>();

    for required in ["research", "read", "generate", "review", "publish"] {
        assert!(
            steps.iter().any(|step| step == &required),
            "missing audit step: {required}"
        );
    }
}
