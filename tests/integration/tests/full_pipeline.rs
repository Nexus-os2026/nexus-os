use nexus_adaptation::adapter::{AdaptationRequest, StrategyAdapter};
use nexus_adaptation::StrategyDocument;
use nexus_analytics::collector::{MetricsCollector, Platform, PlatformMetricsProvider, RawMetric};
use nexus_analytics::evaluator::{PerformanceEvaluator, TemplateAnalyzer};
use nexus_analytics::report::{ReportGenerator, ReportWindow};
use nexus_connectors_web::reader::CleanContent;
use nexus_connectors_web::search::SearchResult;
use nexus_content::generator::{PlatformContent, SocialPlatform};
use nexus_factory::approval::ApprovalFlow;
use nexus_factory::capabilities::map_intent_to_capabilities;
use nexus_factory::code_gen::generate_agent_code;
use nexus_factory::intent::{ParsedIntent, TaskType};
use nexus_factory::manifest_gen::generate_manifest_toml;
use nexus_kernel::errors::AgentError;
use nexus_research::pipeline::{ResearchDataSource, ResearchPipeline};
use nexus_workflows::sequential::{ContentGeneratorPort, ReviewGatePort, SequentialWorkflow};

struct MockResearchSource;

impl ResearchDataSource for MockResearchSource {
    fn search(&mut self, topic: &str, max_results: usize) -> Result<Vec<SearchResult>, AgentError> {
        Ok((0..max_results.min(3))
            .map(|idx| SearchResult {
                title: format!("{topic} result {idx}"),
                url: format!("https://example.com/{idx}"),
                snippet: format!("snippet {idx}"),
                relevance_score: 0.9,
            })
            .collect())
    }

    fn read(&mut self, url: &str) -> Result<CleanContent, AgentError> {
        Ok(CleanContent {
            title: format!("Title {url}"),
            text: format!("Detailed content body for {url} with actionable insights."),
            word_count: 10,
            source_url: url.to_string(),
            extracted_at: 100,
        })
    }
}

struct MockWorkflowGenerator;

impl ContentGeneratorPort for MockWorkflowGenerator {
    fn generate(
        &mut self,
        platform: SocialPlatform,
        topic: &str,
        _style: &str,
    ) -> Result<PlatformContent, AgentError> {
        Ok(PlatformContent {
            platform,
            text: format!("{topic} update for {platform:?}"),
            hashtags: vec!["#nexus".to_string(), "#rust".to_string()],
            thread: None,
            image_prompt: None,
            link_preview: None,
        })
    }
}

struct AlwaysApprove;

impl ReviewGatePort for AlwaysApprove {
    fn approve(&mut self, _strategy_summary: &str) -> bool {
        true
    }
}

struct WorkflowMetricsProvider {
    records: Vec<RawMetric>,
}

impl PlatformMetricsProvider for WorkflowMetricsProvider {
    fn id(&self) -> &str {
        "workflow-metrics"
    }

    fn platform(&self) -> Platform {
        Platform::X
    }

    fn min_poll_interval_secs(&self) -> u64 {
        1
    }

    fn poll_metrics(&mut self) -> Result<Vec<RawMetric>, AgentError> {
        Ok(self.records.clone())
    }
}

#[test]
fn test_full_pipeline_integration() {
    let intent = ParsedIntent {
        task_type: TaskType::ContentPosting,
        platforms: vec!["twitter".to_string()],
        schedule: "daily".to_string(),
        content_topic: "Rust governance".to_string(),
        raw_request: "Create daily Rust governance content agent".to_string(),
    };

    let capability_plan = map_intent_to_capabilities(&intent);
    let manifest = generate_manifest_toml(&intent, &capability_plan);
    let code = generate_agent_code(&intent);
    let mut approval = ApprovalFlow::new();
    let request =
        approval.present_for_review(capability_plan.required.clone(), manifest.fuel_budget);
    let deployed = approval
        .approve_and_deploy(&request, manifest.toml.as_str(), &code, true)
        .expect("agent deployment should succeed");
    assert!(deployed.deployed);

    let mut research = ResearchPipeline::new(MockResearchSource);
    let research_report = research
        .research("Rust governance", 200)
        .expect("research stage should succeed");
    assert!(!research_report.citations.is_empty());

    let mut workflow = SequentialWorkflow::new();
    let mut generator = MockWorkflowGenerator;
    let mut review = AlwaysApprove;
    let workflow_report = workflow.execute(
        &mut generator,
        &mut review,
        &research_report,
        "Rust governance",
        "educational",
        &[SocialPlatform::X, SocialPlatform::Instagram],
    );
    assert!(workflow_report.successes >= 1);
    assert!(!workflow_report.outcomes.is_empty());

    let raw_metrics = workflow_report
        .outcomes
        .iter()
        .map(|outcome| RawMetric {
            content_id: outcome.post_id.clone(),
            like_count: 100,
            retweet_count: 20,
            reply_count: 10,
            comment_count: 10,
            follower_growth: 5,
            content_type: "tutorial".to_string(),
            time_slot: "9am".to_string(),
        })
        .collect::<Vec<_>>();

    let mut collector = MetricsCollector::new();
    collector
        .register_provider(Box::new(WorkflowMetricsProvider {
            records: raw_metrics.clone(),
        }))
        .expect("metrics provider should register");
    let collected = collector
        .collect_scheduled()
        .expect("metrics collection should succeed");
    assert_eq!(collected.len(), raw_metrics.len());

    let evaluator = PerformanceEvaluator::new(TemplateAnalyzer);
    let evaluation = evaluator
        .evaluate(collected.as_slice())
        .expect("performance evaluation should succeed");
    assert!(!evaluation.recommendations.is_empty());

    let generator = ReportGenerator::new(Box::new(|| 12345));
    let report = generator.generate(ReportWindow::Weekly, collected.as_slice(), &evaluation);
    let dashboard = generator
        .render_dashboard(&report)
        .expect("report rendering should succeed");
    assert!(dashboard.contains("top_posts"));

    let mut adapter = StrategyAdapter::new(uuid::Uuid::new_v4());
    let strategy = StrategyDocument {
        posting_times: vec!["2pm".to_string()],
        content_style: "generic".to_string(),
        hashtags: vec!["#governed".to_string()],
        platforms: vec!["x".to_string()],
        weekly_budget: 1_000,
        capabilities: vec!["social.x.post".to_string()],
        fuel_budget: 5_000,
        audit_level: "strict".to_string(),
    };
    let adapted = adapter
        .adapt(&report, &strategy, AdaptationRequest::default())
        .expect("adaptation should succeed");
    assert!(!adapted.updated_strategy.posting_times.is_empty());
}
