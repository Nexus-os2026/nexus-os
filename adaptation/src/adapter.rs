use crate::authority::{classify_change, is_never_allowed, ChangeClass, StrategyChange};
use crate::{AdaptationError, StrategyDocument};
use analytics::report::AnalyticsReport;
use nexus_connectors_llm::providers::LlmProvider;
use nexus_kernel::audit::{AuditTrail, EventType};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptationRequest {
    pub new_platforms: Vec<String>,
    pub budget_increase: Option<u64>,
    pub new_capabilities: Vec<String>,
    pub fuel_override: Option<u64>,
    pub audit_bypass_requested: bool,
    pub approval_granted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptationResult {
    pub updated_strategy: StrategyDocument,
    pub applied_changes: Vec<StrategyChange>,
    pub authority_changes: Vec<StrategyChange>,
}

pub struct StrategyAdapter {
    agent_id: Uuid,
    audit_trail: AuditTrail,
}

impl StrategyAdapter {
    pub fn new(agent_id: Uuid) -> Self {
        Self {
            agent_id,
            audit_trail: AuditTrail::new(),
        }
    }

    pub fn adapt(
        &mut self,
        analytics_report: &AnalyticsReport,
        current_strategy: &StrategyDocument,
        request: AdaptationRequest,
    ) -> Result<AdaptationResult, AdaptationError> {
        let mut updated_strategy = current_strategy.clone();
        let mut applied_changes = Vec::new();

        if let Some(new_time) = extract_preferred_time(analytics_report) {
            let current_preferred = updated_strategy
                .posting_times
                .first()
                .cloned()
                .unwrap_or_else(|| "unset".to_string());
            if current_preferred != new_time {
                updated_strategy
                    .posting_times
                    .retain(|slot| !slot.eq_ignore_ascii_case(new_time.as_str()));
                updated_strategy.posting_times.insert(0, new_time.clone());
                applied_changes.push(StrategyChange::PostingTimeUpdate {
                    from: current_preferred,
                    to: new_time,
                });
            }
        }

        if let Some(style) = extract_preferred_style(analytics_report) {
            if !updated_strategy.content_style.eq_ignore_ascii_case(style.as_str()) {
                let previous_style = updated_strategy.content_style.clone();
                updated_strategy.content_style = style.clone();
                applied_changes.push(StrategyChange::ContentStyleUpdate {
                    from: previous_style,
                    to: style,
                });
            }
        }

        let additional_hashtags = extract_hashtags(analytics_report);
        if !additional_hashtags.is_empty() {
            let mut added = Vec::new();
            for hashtag in additional_hashtags {
                let present = updated_strategy
                    .hashtags
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(hashtag.as_str()));
                if !present {
                    updated_strategy.hashtags.push(hashtag.clone());
                    added.push(hashtag);
                }
            }

            if !added.is_empty() {
                applied_changes.push(StrategyChange::HashtagUpdate { added });
            }
        }

        let authority_changes = collect_authority_changes(current_strategy, &request);
        for change in &authority_changes {
            if is_never_allowed(change) {
                self.log_security_event(change, "never_allowed");
                return Err(AdaptationError::NeverAllowed(format!(
                    "{}",
                    change_description(change)
                )));
            }
        }

        if !authority_changes.is_empty() && !request.approval_granted {
            let pending = authority_changes
                .iter()
                .map(change_description)
                .collect::<Vec<_>>()
                .join(", ");
            let _ = self.audit_trail.append_event(
                self.agent_id,
                EventType::UserAction,
                json!({
                    "event": "adaptation_blocked_pending_approval",
                    "pending_changes": pending
                }),
            );
            return Err(AdaptationError::RequiresApproval(pending));
        }

        for change in &authority_changes {
            apply_authority_change(change, &mut updated_strategy);
        }

        updated_strategy.normalize();

        let _ = self.audit_trail.append_event(
            self.agent_id,
            EventType::UserAction,
            json!({
                "event": "auto_adaptation_applied",
                "strategy_edits": applied_changes.iter().filter(|change| classify_change(change) == ChangeClass::StrategyEdit).count(),
                "authority_changes": authority_changes.len(),
                "report_window": format!("{:?}", analytics_report.window)
            }),
        );

        Ok(AdaptationResult {
            updated_strategy,
            applied_changes,
            authority_changes,
        })
    }

    pub fn audit_trail(&self) -> &AuditTrail {
        &self.audit_trail
    }

    fn log_security_event(&mut self, change: &StrategyChange, policy: &str) {
        let _ = self.audit_trail.append_event(
            self.agent_id,
            EventType::Error,
            json!({
                "event": "adaptation_security_violation",
                "policy": policy,
                "change": change_description(change)
            }),
        );
    }
}

pub struct LlmStrategyAdvisor<P: LlmProvider> {
    provider: P,
    model: String,
}

impl<P: LlmProvider> LlmStrategyAdvisor<P> {
    pub fn new(provider: P, model: impl Into<String>) -> Self {
        Self {
            provider,
            model: model.into(),
        }
    }

    pub fn recommend_focus(
        &self,
        analytics_report: &AnalyticsReport,
        current_strategy: &StrategyDocument,
    ) -> Result<Vec<String>, AdaptationError> {
        let prompt = format!(
            "Given report summary '{}' and strategy style '{}', return one recommendation per line.",
            analytics_report.llm_summary,
            current_strategy.content_style
        );

        let response = self
            .provider
            .query(prompt.as_str(), 64, self.model.as_str())
            .map_err(AdaptationError::from)?;

        let recommendations = response
            .output_text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>();

        Ok(recommendations)
    }
}

fn collect_authority_changes(
    current_strategy: &StrategyDocument,
    request: &AdaptationRequest,
) -> Vec<StrategyChange> {
    let mut changes = Vec::new();

    let mut new_platforms = request.new_platforms.clone();
    new_platforms.sort();
    new_platforms.dedup();
    for platform in new_platforms {
        let already_exists = current_strategy
            .platforms
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(platform.as_str()));
        if !already_exists {
            changes.push(StrategyChange::AddPlatform { platform });
        }
    }

    if let Some(new_budget) = request.budget_increase {
        if new_budget > current_strategy.weekly_budget {
            changes.push(StrategyChange::IncreaseBudget {
                from: current_strategy.weekly_budget,
                to: new_budget,
            });
        }
    }

    let mut requested_capabilities = request.new_capabilities.clone();
    requested_capabilities.sort();
    requested_capabilities.dedup();
    for capability in requested_capabilities {
        let already_exists = current_strategy
            .capabilities
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(capability.as_str()));
        if !already_exists {
            changes.push(StrategyChange::AddCapability { capability });
        }
    }

    if let Some(override_fuel) = request.fuel_override {
        if override_fuel != current_strategy.fuel_budget {
            changes.push(StrategyChange::FuelOverride {
                from: current_strategy.fuel_budget,
                to: override_fuel,
            });
        }
    }

    if request.audit_bypass_requested {
        changes.push(StrategyChange::AuditBypassRequested);
    }

    changes
}

fn apply_authority_change(change: &StrategyChange, strategy: &mut StrategyDocument) {
    match change {
        StrategyChange::AddPlatform { platform } => {
            strategy.platforms.push(platform.clone());
        }
        StrategyChange::IncreaseBudget { to, .. } => {
            strategy.weekly_budget = *to;
        }
        StrategyChange::PostingTimeUpdate { .. }
        | StrategyChange::ContentStyleUpdate { .. }
        | StrategyChange::HashtagUpdate { .. }
        | StrategyChange::AddCapability { .. }
        | StrategyChange::FuelOverride { .. }
        | StrategyChange::AuditBypassRequested => {}
    }
}

fn change_description(change: &StrategyChange) -> String {
    match change {
        StrategyChange::PostingTimeUpdate { from, to } => {
            format!("posting_time_update:{from}->{to}")
        }
        StrategyChange::ContentStyleUpdate { from, to } => {
            format!("content_style_update:{from}->{to}")
        }
        StrategyChange::HashtagUpdate { added } => {
            format!("hashtag_update:{}", added.join("|"))
        }
        StrategyChange::AddPlatform { platform } => {
            format!("add_platform:{platform}")
        }
        StrategyChange::IncreaseBudget { from, to } => {
            format!("increase_budget:{from}->{to}")
        }
        StrategyChange::AddCapability { capability } => {
            format!("add_capability:{capability}")
        }
        StrategyChange::FuelOverride { from, to } => {
            format!("fuel_override:{from}->{to}")
        }
        StrategyChange::AuditBypassRequested => "audit_bypass_requested".to_string(),
    }
}

fn extract_preferred_time(report: &AnalyticsReport) -> Option<String> {
    if let Ok(regex) = regex::Regex::new(r"(?i)\b([0-1]?\d|2[0-3])\s?(am|pm)\b") {
        for source in report
            .recommendations
            .iter()
            .chain(report.growth_trends.iter())
            .chain(std::iter::once(&report.llm_summary))
        {
            if let Some(captures) = regex.captures(source) {
                let hour = captures.get(1).map(|value| value.as_str()).unwrap_or("");
                let suffix = captures.get(2).map(|value| value.as_str()).unwrap_or("");
                if !hour.is_empty() && !suffix.is_empty() {
                    return Some(format!("{}{}", hour, suffix.to_lowercase()));
                }
            }
        }
    }

    None
}

fn extract_preferred_style(report: &AnalyticsReport) -> Option<String> {
    let corpus = report
        .recommendations
        .iter()
        .chain(report.growth_trends.iter())
        .chain(std::iter::once(&report.llm_summary))
        .map(|value| value.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");

    let candidates = [
        ("tutorial", "tutorial"),
        ("thread", "thread"),
        ("carousel", "carousel"),
        ("video", "video"),
        ("long-form", "long-form"),
    ];

    for (needle, style) in candidates {
        if corpus.contains(needle) {
            return Some(style.to_string());
        }
    }

    None
}

fn extract_hashtags(report: &AnalyticsReport) -> Vec<String> {
    let mut hashtags = Vec::new();
    if let Ok(regex) = regex::Regex::new(r"#[A-Za-z0-9_]{2,32}") {
        for source in report
            .recommendations
            .iter()
            .chain(report.growth_trends.iter())
            .chain(std::iter::once(&report.llm_summary))
        {
            for candidate in regex.find_iter(source) {
                hashtags.push(candidate.as_str().to_lowercase());
            }
        }
    }
    hashtags.sort();
    hashtags.dedup();
    hashtags
}

#[cfg(test)]
mod tests {
    use super::{AdaptationRequest, StrategyAdapter};
    use crate::AdaptationError;
    use crate::StrategyDocument;
    use analytics::collector::Platform;
    use analytics::evaluator::ScoredPost;
    use analytics::report::{AnalyticsReport, ReportWindow};
    use nexus_kernel::audit::EventType;
    use uuid::Uuid;

    fn base_report() -> AnalyticsReport {
        AnalyticsReport {
            window: ReportWindow::Weekly,
            generated_at: 1,
            top_posts: vec![ScoredPost {
                platform: Platform::X,
                content_id: "tweet-1".to_string(),
                score: 300,
                follower_growth: 15,
            }],
            worst_posts: Vec::new(),
            growth_trends: vec!["x follower_growth=21 over 10 observations".to_string()],
            recommendations: vec![
                "9am posts get 3x engagement compared with 2pm".to_string(),
                "Use tutorial style and #rust #nexusos".to_string(),
            ],
            llm_summary: "tutorial content outperforms".to_string(),
        }
    }

    fn base_strategy() -> StrategyDocument {
        StrategyDocument {
            posting_times: vec!["2pm".to_string()],
            content_style: "generic".to_string(),
            hashtags: vec!["#governedai".to_string()],
            platforms: vec!["x".to_string()],
            weekly_budget: 1_000,
            capabilities: vec!["social.x.post".to_string()],
            fuel_budget: 5_000,
            audit_level: "strict".to_string(),
        }
    }

    #[test]
    fn test_auto_adapt_posting_time() {
        let mut adapter = StrategyAdapter::new(Uuid::new_v4());
        let report = base_report();
        let strategy = base_strategy();

        let result = adapter.adapt(&report, &strategy, AdaptationRequest::default());
        assert!(result.is_ok());

        if let Ok(result) = result {
            assert_eq!(result.updated_strategy.posting_times.first(), Some(&"9am".to_string()));
            assert!(result.authority_changes.is_empty());

            let has_auto_event = adapter.audit_trail().events().iter().any(|event| {
                event.event_type == EventType::UserAction
                    && event
                        .payload
                        .get("event")
                        .and_then(|value| value.as_str())
                        == Some("auto_adaptation_applied")
            });
            assert!(has_auto_event);
        }
    }

    #[test]
    fn test_authority_change_requires_approval() {
        let mut adapter = StrategyAdapter::new(Uuid::new_v4());
        let report = base_report();
        let strategy = base_strategy();

        let request = AdaptationRequest {
            new_platforms: vec!["instagram".to_string()],
            ..AdaptationRequest::default()
        };

        let result = adapter.adapt(&report, &strategy, request);
        assert!(matches!(result, Err(AdaptationError::RequiresApproval(_))));
    }

    #[test]
    fn test_never_allowed_escalation() {
        let mut adapter = StrategyAdapter::new(Uuid::new_v4());
        let report = base_report();
        let strategy = base_strategy();

        let request = AdaptationRequest {
            new_capabilities: vec!["social.instagram.post".to_string()],
            ..AdaptationRequest::default()
        };

        let result = adapter.adapt(&report, &strategy, request);
        assert!(matches!(result, Err(AdaptationError::NeverAllowed(_))));

        let security_logged = adapter.audit_trail().events().iter().any(|event| {
            event.event_type == EventType::Error
                && event
                    .payload
                    .get("event")
                    .and_then(|value| value.as_str())
                    == Some("adaptation_security_violation")
        });
        assert!(security_logged);
    }
}
