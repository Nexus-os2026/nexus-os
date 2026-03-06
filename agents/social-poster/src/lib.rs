//! Social poster agent pipeline for governed research, generation, review, and publishing.

use nexus_connectors_llm::gateway::{select_provider, ProviderSelectionConfig};
use nexus_connectors_llm::providers::LlmProvider;
use nexus_connectors_llm::providers::MockProvider;
use nexus_connectors_web::reader::{CleanContent, WebReaderConnector};
use nexus_connectors_web::search::{FallbackProvider, SearchResult, WebSearchConnector};
use nexus_connectors_web::twitter::{TweetResult, TwitterConnector};
use nexus_connectors_web::WebAgentContext;
use nexus_content::compliance::{check_compliance, ComplianceDecision};
use nexus_content::generator::{ContentGenerator, PlatformContent, SocialPlatform};
use nexus_sdk::audit::{AuditEvent, AuditTrail, EventType};
use nexus_sdk::autonomy::{AutonomyGuard, AutonomyLevel};
use nexus_sdk::config::load_config;
use nexus_sdk::consent::{ApprovalRequest, ConsentRuntime, GovernedOperation};
use nexus_sdk::errors::AgentError;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
/// Runtime configuration values loaded from the agent manifest `[config]` section.
pub struct SocialPosterConfig {
    pub topic: String,
    pub platforms: Vec<String>,
    pub style: String,
    pub posts_per_day: u32,
}

#[derive(Debug, Clone, Deserialize)]
/// Typed manifest model for the `social-poster` agent package.
pub struct SocialPosterManifest {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub fuel_budget: u64,
    #[serde(default)]
    pub autonomy_level: Option<u8>,
    #[serde(default)]
    pub consent_policy_path: Option<String>,
    #[serde(default)]
    pub requester_id: Option<String>,
    pub schedule: Option<String>,
    pub llm_model: Option<String>,
    pub config: SocialPosterConfig,
}

#[derive(Debug, Clone)]
/// Execution summary returned after a social-poster run completes.
pub struct SocialPosterRunReport {
    pub generated_posts: Vec<PlatformContent>,
    pub published_post_ids: Vec<String>,
    pub dry_run: bool,
    pub publish_calls: usize,
    pub audit_events: Vec<AuditEvent>,
}

/// Research stage abstraction (search provider).
pub trait SearchStep {
    fn search(&mut self, topic: &str, max_results: usize) -> Result<Vec<SearchResult>, AgentError>;
}

/// Reading stage abstraction (web content extraction provider).
pub trait ReaderStep {
    fn read(&mut self, url: &str) -> Result<CleanContent, AgentError>;
}

/// Generation stage abstraction (LLM/content provider).
pub trait GenerateStep {
    fn generate(
        &mut self,
        platform: SocialPlatform,
        topic: &str,
        style: &str,
    ) -> Result<PlatformContent, AgentError>;
}

/// Publishing stage abstraction (social API provider).
pub trait PublishStep {
    fn publish_x(&mut self, text: &str) -> Result<TweetResult, AgentError>;
    fn publish_calls(&self) -> usize;
}

/// Dependency bundle for social-poster pipeline stages.
pub struct PipelineDependencies {
    pub search: Box<dyn SearchStep>,
    pub reader: Box<dyn ReaderStep>,
    pub generator: Box<dyn GenerateStep>,
    pub publisher: Box<dyn PublishStep>,
}

impl PipelineDependencies {
    /// Builds production dependencies backed by real connectors/providers.
    pub fn real(fuel_budget: u64, model_name: &str) -> Result<Self, AgentError> {
        Ok(Self {
            search: Box::new(RealSearchStep::new(fuel_budget)),
            reader: Box::new(RealReaderStep::new(fuel_budget)),
            generator: Box::new(RealGenerateStep::new(model_name, fuel_budget)?),
            publisher: Box::new(RealPublishStep::new(fuel_budget)),
        })
    }

    /// Builds offline-safe dry-run dependencies that avoid external mutations.
    pub fn dry_run_defaults(model_name: &str, fuel_budget: u64) -> Self {
        Self {
            search: Box::new(DryRunSearchStep),
            reader: Box::new(DryRunReaderStep),
            generator: Box::new(DryRunGenerateStep {
                generator: ContentGenerator::new(
                    Box::new(MockProvider::new()),
                    model_name,
                    fuel_budget,
                ),
            }),
            publisher: Box::new(DryRunPublishStep { calls: 0 }),
        }
    }
}

/// Governed runnable social-poster agent.
pub struct SocialPosterAgent {
    manifest: SocialPosterManifest,
    dependencies: PipelineDependencies,
    dry_run: bool,
    agent_id: Uuid,
    audit_trail: AuditTrail,
    autonomy_guard: AutonomyGuard,
    consent_runtime: Option<ConsentRuntime>,
}

impl SocialPosterAgent {
    /// Creates a social-poster agent from manifest values.
    pub fn new(manifest: SocialPosterManifest, dry_run: bool) -> Result<Self, AgentError> {
        let model_name = manifest
            .llm_model
            .clone()
            .unwrap_or_else(|| "claude-sonnet-4-5".to_string());
        let dependencies = if dry_run {
            PipelineDependencies::dry_run_defaults(model_name.as_str(), manifest.fuel_budget)
        } else {
            PipelineDependencies::real(manifest.fuel_budget, model_name.as_str())?
        };
        Ok(Self::with_dependencies(manifest, dry_run, dependencies))
    }

    /// Creates a social-poster agent with injected dependencies (useful for tests).
    pub fn with_dependencies(
        manifest: SocialPosterManifest,
        dry_run: bool,
        dependencies: PipelineDependencies,
    ) -> Self {
        Self {
            autonomy_guard: AutonomyGuard::new(AutonomyLevel::from_manifest(
                manifest.autonomy_level,
            )),
            manifest,
            dependencies,
            dry_run,
            agent_id: Uuid::new_v4(),
            audit_trail: AuditTrail::new(),
            consent_runtime: None,
        }
    }

    /// Executes the full pipeline and returns a run report with audit events.
    pub fn run(&mut self) -> Result<SocialPosterRunReport, AgentError> {
        self.audit_trail.append_event(
            self.agent_id,
            EventType::StateChange,
            json!({
                "step": "start",
                "name": self.manifest.name,
                "version": self.manifest.version,
                "schedule": self.manifest.schedule,
                "topic": self.manifest.config.topic,
                "posts_per_day": self.manifest.config.posts_per_day,
                "dry_run": self.dry_run,
                "autonomy_level": self.autonomy_guard.level().as_str(),
            }),
        );

        let search_query = format!("latest {} news", self.manifest.config.topic);
        self.require_operation(GovernedOperation::ToolCall, search_query.as_bytes())?;
        let search_results = self.dependencies.search.search(search_query.as_str(), 8)?;
        self.audit_trail.append_event(
            self.agent_id,
            EventType::ToolCall,
            json!({
                "step": "research",
                "query": search_query,
                "results": search_results.len()
            }),
        );

        let mut key_points = Vec::new();
        for result in search_results.into_iter().take(3) {
            self.require_operation(GovernedOperation::ToolCall, result.url.as_bytes())?;
            let content = self.dependencies.reader.read(result.url.as_str())?;
            let summary = summarize(content.text.as_str(), 220);
            self.audit_trail.append_event(
                self.agent_id,
                EventType::ToolCall,
                json!({
                    "step": "read",
                    "url": result.url,
                    "title": content.title,
                    "summary": summary
                }),
            );
            key_points.push(summary);
        }

        let mut generated_posts = Vec::new();
        let mut published_post_ids = Vec::new();
        let synthesis = if key_points.is_empty() {
            "No readable articles were found.".to_string()
        } else {
            key_points.join(" | ")
        };

        let posts_target = self.manifest.config.posts_per_day.max(1);
        let platforms = self.manifest.config.platforms.clone();
        for platform_label in platforms {
            let Some(platform) = parse_platform(platform_label.as_str()) else {
                self.audit_trail.append_event(
                    self.agent_id,
                    EventType::Error,
                    json!({
                        "step": "platform",
                        "status": "unsupported",
                        "platform": platform_label.as_str()
                    }),
                );
                continue;
            };

            for slot in 0..posts_target {
                let generation_topic =
                    format!("{}. Key points: {}", self.manifest.config.topic, synthesis);
                self.require_operation(GovernedOperation::ToolCall, generation_topic.as_bytes())?;
                let generated = self.dependencies.generator.generate(
                    platform,
                    generation_topic.as_str(),
                    self.manifest.config.style.as_str(),
                )?;
                self.audit_trail.append_event(
                    self.agent_id,
                    EventType::LlmCall,
                    json!({
                        "step": "generate",
                        "platform": platform_label.as_str(),
                        "slot": slot,
                        "length": generated.text.chars().count()
                    }),
                );

                let compliance = check_compliance(platform, slot as usize);
                self.audit_trail.append_event(
                    self.agent_id,
                    EventType::ToolCall,
                    json!({
                        "step": "review",
                        "platform": platform_label.as_str(),
                        "slot": slot,
                        "decision": format!("{compliance:?}")
                    }),
                );
                if let ComplianceDecision::Blocked(reason) = compliance {
                    self.audit_trail.append_event(
                        self.agent_id,
                        EventType::Error,
                        json!({
                            "step": "review",
                            "status": "blocked",
                            "reason": reason
                        }),
                    );
                    continue;
                }

                if self.dry_run {
                    self.audit_trail.append_event(
                        self.agent_id,
                        EventType::ToolCall,
                        json!({
                            "step": "publish",
                            "mode": "dry-run",
                            "platform": platform_label.as_str(),
                            "slot": slot,
                            "content": generated.text
                        }),
                    );
                    generated_posts.push(generated);
                    continue;
                }

                match platform {
                    SocialPlatform::X => {
                        self.require_operation(
                            GovernedOperation::SocialPostPublish,
                            generated.text.as_bytes(),
                        )?;
                        let publish_result = self
                            .dependencies
                            .publisher
                            .publish_x(generated.text.as_str())?;
                        self.audit_trail.append_event(
                            self.agent_id,
                            EventType::ToolCall,
                            json!({
                                "step": "publish",
                                "mode": "live",
                                "platform": platform_label.as_str(),
                                "slot": slot,
                                "tweet_id": publish_result.tweet_id
                            }),
                        );
                        published_post_ids.push(publish_result.tweet_id);
                        generated_posts.push(generated);
                    }
                    SocialPlatform::Instagram | SocialPlatform::Facebook => {
                        self.audit_trail.append_event(
                            self.agent_id,
                            EventType::Error,
                            json!({
                                "step": "publish",
                                "status": "skipped",
                                "reason": "platform publisher not wired yet",
                                "platform": platform_label.as_str()
                            }),
                        );
                    }
                }
            }
        }

        self.audit_trail.append_event(
            self.agent_id,
            EventType::StateChange,
            json!({
                "step": "complete",
                "generated_posts": generated_posts.len(),
                "published_posts": published_post_ids.len(),
                "dry_run": self.dry_run
            }),
        );

        Ok(SocialPosterRunReport {
            generated_posts,
            published_post_ids,
            dry_run: self.dry_run,
            publish_calls: self.dependencies.publisher.publish_calls(),
            audit_events: self.audit_trail.events().to_vec(),
        })
    }

    fn require_operation(
        &mut self,
        operation: GovernedOperation,
        payload: &[u8],
    ) -> Result<(), AgentError> {
        let agent_id = self.agent_id;
        self.autonomy_guard
            .require_tool_call(self.agent_id, &mut self.audit_trail)
            .map_err(AgentError::from)?;
        self.with_consent_runtime(|runtime, audit_trail| {
            runtime
                .enforce_operation(operation, agent_id, payload, audit_trail)
                .map_err(AgentError::from)
        })
    }

    fn ensure_consent_runtime(&mut self) -> Result<(), AgentError> {
        if self.consent_runtime.is_none() {
            self.consent_runtime = Some(ConsentRuntime::from_manifest(
                self.manifest.consent_policy_path.as_deref(),
                self.manifest.requester_id.as_deref(),
                self.manifest.name.as_str(),
            )?);
        }
        Ok(())
    }

    fn with_consent_runtime<T>(
        &mut self,
        f: impl FnOnce(&mut ConsentRuntime, &mut AuditTrail) -> Result<T, AgentError>,
    ) -> Result<T, AgentError> {
        self.ensure_consent_runtime()?;
        let mut runtime = self.consent_runtime.take().ok_or_else(|| {
            AgentError::SupervisorError("consent runtime was not initialized".to_string())
        })?;
        let result = f(&mut runtime, &mut self.audit_trail);
        self.consent_runtime = Some(runtime);
        result
    }

    pub fn pending_approvals(&self) -> Vec<ApprovalRequest> {
        match &self.consent_runtime {
            Some(runtime) => runtime.pending_requests(),
            None => Vec::new(),
        }
    }

    pub fn approve_request(
        &mut self,
        request_id: &str,
        approver_id: &str,
    ) -> Result<(), AgentError> {
        self.with_consent_runtime(|runtime, audit_trail| {
            runtime
                .approve(request_id, approver_id, audit_trail)
                .map_err(AgentError::from)
        })
    }

    pub fn deny_request(&mut self, request_id: &str, approver_id: &str) -> Result<(), AgentError> {
        self.with_consent_runtime(|runtime, audit_trail| {
            runtime
                .deny(request_id, approver_id, audit_trail)
                .map_err(AgentError::from)
        })
    }
}

/// Loads and deserializes a social-poster manifest from disk.
pub fn load_manifest(path: &Path) -> Result<SocialPosterManifest, AgentError> {
    let manifest_str = fs::read_to_string(path).map_err(|error| {
        AgentError::ManifestError(format!(
            "unable to read manifest '{}': {error}",
            path.display()
        ))
    })?;
    toml::from_str::<SocialPosterManifest>(manifest_str.as_str())
        .map_err(|error| AgentError::ManifestError(format!("invalid manifest format: {error}")))
}

/// Runs social-poster directly from a manifest path.
pub fn run_social_poster_from_manifest(
    manifest_path: &Path,
    dry_run: bool,
) -> Result<SocialPosterRunReport, AgentError> {
    let manifest = load_manifest(manifest_path)?;
    let mut agent = SocialPosterAgent::new(manifest, dry_run)?;
    agent.run()
}

fn summarize(input: &str, max_chars: usize) -> String {
    let compact = input.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars = compact.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return compact;
    }
    chars.into_iter().take(max_chars).collect::<String>()
}

fn parse_platform(value: &str) -> Option<SocialPlatform> {
    match value.trim().to_ascii_lowercase().as_str() {
        "x" | "twitter" => Some(SocialPlatform::X),
        "instagram" => Some(SocialPlatform::Instagram),
        "facebook" => Some(SocialPlatform::Facebook),
        _ => None,
    }
}

struct RealSearchStep {
    connector: WebSearchConnector,
    context: WebAgentContext,
}

impl RealSearchStep {
    fn new(fuel_budget: u64) -> Self {
        Self {
            connector: WebSearchConnector::new(FallbackProvider::None),
            context: WebAgentContext::new(
                Uuid::new_v4(),
                ["web.search".to_string()]
                    .into_iter()
                    .collect::<HashSet<_>>(),
                fuel_budget,
            ),
        }
    }
}

impl SearchStep for RealSearchStep {
    fn search(&mut self, topic: &str, max_results: usize) -> Result<Vec<SearchResult>, AgentError> {
        self.connector.query(&mut self.context, topic, max_results)
    }
}

struct RealReaderStep {
    connector: WebReaderConnector,
    context: WebAgentContext,
}

impl RealReaderStep {
    fn new(fuel_budget: u64) -> Self {
        Self {
            connector: WebReaderConnector::new(None),
            context: WebAgentContext::new(
                Uuid::new_v4(),
                ["web.read".to_string()].into_iter().collect::<HashSet<_>>(),
                fuel_budget,
            ),
        }
    }
}

impl ReaderStep for RealReaderStep {
    fn read(&mut self, url: &str) -> Result<CleanContent, AgentError> {
        self.connector.fetch_and_extract(&mut self.context, url)
    }
}

struct RealGenerateStep {
    generator: ContentGenerator<Box<dyn LlmProvider>>,
}

impl RealGenerateStep {
    fn new(model_name: &str, fuel_budget: u64) -> Result<Self, AgentError> {
        let config = load_config()?;
        let provider_config = ProviderSelectionConfig {
            provider: std::env::var("LLM_PROVIDER").ok(),
            ollama_url: if config.llm.ollama_url.trim().is_empty() {
                None
            } else {
                Some(config.llm.ollama_url.clone())
            },
            deepseek_api_key: std::env::var("DEEPSEEK_API_KEY").ok(),
            anthropic_api_key: if config.llm.anthropic_api_key.trim().is_empty() {
                None
            } else {
                Some(config.llm.anthropic_api_key.clone())
            },
        };
        let provider = select_provider(&provider_config);
        Ok(Self {
            generator: ContentGenerator::new(provider, model_name, fuel_budget),
        })
    }
}

impl GenerateStep for RealGenerateStep {
    fn generate(
        &mut self,
        platform: SocialPlatform,
        topic: &str,
        style: &str,
    ) -> Result<PlatformContent, AgentError> {
        self.generator.generate_post(platform, topic, style)
    }
}

struct RealPublishStep {
    connector: TwitterConnector,
    context: WebAgentContext,
    calls: usize,
}

impl RealPublishStep {
    fn new(fuel_budget: u64) -> Self {
        Self {
            connector: TwitterConnector::new(),
            context: WebAgentContext::new(
                Uuid::new_v4(),
                ["social.x.post".to_string(), "social.x.read".to_string()]
                    .into_iter()
                    .collect::<HashSet<_>>(),
                fuel_budget,
            ),
            calls: 0,
        }
    }
}

impl PublishStep for RealPublishStep {
    fn publish_x(&mut self, text: &str) -> Result<TweetResult, AgentError> {
        self.calls += 1;
        self.connector.post_status_update(&mut self.context, text)
    }

    fn publish_calls(&self) -> usize {
        self.calls
    }
}

struct DryRunSearchStep;

impl SearchStep for DryRunSearchStep {
    fn search(&mut self, topic: &str, max_results: usize) -> Result<Vec<SearchResult>, AgentError> {
        Ok((0..max_results.min(5))
            .map(|index| SearchResult {
                title: format!("{topic} briefing #{index}"),
                url: format!("https://dry-run.local/article-{index}"),
                snippet: format!("synthetic trend insight {index}"),
                relevance_score: 0.95 - (index as f32 * 0.1),
            })
            .collect())
    }
}

struct DryRunReaderStep;

impl ReaderStep for DryRunReaderStep {
    fn read(&mut self, url: &str) -> Result<CleanContent, AgentError> {
        Ok(CleanContent {
            title: format!("Dry-run source {url}"),
            text: format!("Dry-run article synthesis extracted from {url}."),
            word_count: 8,
            source_url: url.to_string(),
            extracted_at: 0,
        })
    }
}

struct DryRunGenerateStep {
    generator: ContentGenerator<Box<dyn LlmProvider>>,
}

impl GenerateStep for DryRunGenerateStep {
    fn generate(
        &mut self,
        platform: SocialPlatform,
        topic: &str,
        style: &str,
    ) -> Result<PlatformContent, AgentError> {
        self.generator.generate_post(platform, topic, style)
    }
}

struct DryRunPublishStep {
    calls: usize,
}

impl PublishStep for DryRunPublishStep {
    fn publish_x(&mut self, _text: &str) -> Result<TweetResult, AgentError> {
        self.calls += 1;
        Ok(TweetResult {
            tweet_id: format!("dry-run-{}", self.calls),
            posted_at: 0,
        })
    }

    fn publish_calls(&self) -> usize {
        self.calls
    }
}
