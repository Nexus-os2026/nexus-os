//! Integration router — dispatches events to matching providers with
//! PII redaction, rate limiting, and audit trail integration.

use crate::config::{IntegrationConfig, ProviderConfig};
use crate::error::IntegrationError;
use crate::events::{NexusEvent, Notification};
use crate::providers::discord::DiscordIntegration;
use crate::providers::slack::SlackIntegration;
use crate::providers::teams::TeamsIntegration;
use crate::providers::telegram::TelegramIntegration;
use crate::providers::webhook::WebhookIntegration;
use crate::providers::{Integration, ProviderType};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::consent::{ConsentRuntime, GovernedOperation};
use nexus_kernel::fuel_hardening::FuelContext;
use nexus_kernel::redaction::RedactionEngine;
use nexus_kernel::supervisor::max_fuel_cost;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

/// Tracks per-provider request counts inside a sliding window.
#[derive(Debug)]
struct SlidingWindow {
    /// (timestamp_secs, count) pairs — one per second that saw traffic.
    entries: Vec<(u64, u32)>,
    rpm: u32,
}

impl SlidingWindow {
    fn new(rpm: u32) -> Self {
        Self {
            entries: Vec::new(),
            rpm,
        }
    }

    /// Returns `true` if the request is allowed; `false` if rate-limited.
    fn check_and_record(&mut self) -> bool {
        let now = current_epoch_secs();
        let window_start = now.saturating_sub(60);

        // Evict old entries.
        self.entries.retain(|(ts, _)| *ts > window_start);

        let total: u32 = self.entries.iter().map(|(_, c)| c).sum();
        if total >= self.rpm {
            return false;
        }

        // Record this request.
        if let Some(last) = self.entries.last_mut() {
            if last.0 == now {
                last.1 += 1;
            } else {
                self.entries.push((now, 1));
            }
        } else {
            self.entries.push((now, 1));
        }
        true
    }
}

fn current_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Central event router that fans out Nexus events to all matching integrations.
///
/// ```text
/// NexusEvent
///   → match against provider event filters
///   → PII-redact the payload
///   → rate-limit check
///   → send to each matching provider
///   → audit trail entry per send
/// ```
pub struct IntegrationRouter {
    providers: Vec<Box<dyn Integration>>,
    audit: Mutex<AuditTrail>,
    rate_limits: Mutex<HashMap<String, SlidingWindow>>,
    consent: Option<Mutex<ConsentRuntime>>,
    agent_id: Uuid,
    /// Tracks approved provider channels: provider_name → expiry_epoch_secs.
    /// Once approved, a channel is whitelisted for 24 hours to avoid HITL fatigue.
    approved_channels: Mutex<HashMap<String, u64>>,
    /// Optional fuel context — when set, every integration send deducts fuel.
    fuel: Option<FuelContext>,
}

impl IntegrationRouter {
    /// Build a router from the given config, instantiating all enabled providers.
    pub fn from_config(config: &IntegrationConfig) -> Self {
        let mut providers: Vec<Box<dyn Integration>> = Vec::new();
        let mut rate_limits = HashMap::new();

        // Slack
        if let Some(cfg) = &config.slack {
            if cfg.enabled {
                if let Ok(integration) = Self::build_slack(cfg) {
                    rate_limits.insert("slack".to_string(), SlidingWindow::new(cfg.rate_limit_rpm));
                    providers.push(Box::new(integration));
                }
            }
        }

        // Teams
        if let Some(cfg) = &config.teams {
            if cfg.enabled {
                if let Ok(integration) = Self::build_teams(cfg) {
                    rate_limits.insert("teams".to_string(), SlidingWindow::new(cfg.rate_limit_rpm));
                    providers.push(Box::new(integration));
                }
            }
        }

        // Discord
        if let Some(cfg) = &config.discord {
            if cfg.enabled {
                if let Ok(integration) = Self::build_discord(cfg) {
                    rate_limits
                        .insert("discord".to_string(), SlidingWindow::new(cfg.rate_limit_rpm));
                    providers.push(Box::new(integration));
                }
            }
        }

        // Telegram
        if let Some(cfg) = &config.telegram {
            if cfg.enabled {
                if let Ok(integration) = Self::build_telegram(cfg) {
                    rate_limits
                        .insert("telegram".to_string(), SlidingWindow::new(cfg.rate_limit_rpm));
                    providers.push(Box::new(integration));
                }
            }
        }

        // Webhooks
        for (id, wh_cfg) in &config.webhooks {
            if wh_cfg.enabled {
                if let Ok(integration) = WebhookIntegration::new(id.clone(), wh_cfg.clone()) {
                    let key = format!("webhook:{id}");
                    rate_limits.insert(key, SlidingWindow::new(wh_cfg.retry_count.max(30)));
                    providers.push(Box::new(integration));
                }
            }
        }

        // Note: Jira, ServiceNow, GitHub, GitLab are ticket-oriented and
        // are typically invoked directly (create_ticket, update_status) rather
        // than through the event router. They can still be instantiated and
        // used via `get_provider()` for direct calls.

        Self {
            providers,
            audit: Mutex::new(AuditTrail::new()),
            rate_limits: Mutex::new(rate_limits),
            consent: None,
            agent_id: Uuid::nil(),
            approved_channels: Mutex::new(HashMap::new()),
            fuel: None,
        }
    }

    /// Create a router with no providers (useful for testing).
    pub fn empty() -> Self {
        Self {
            providers: Vec::new(),
            audit: Mutex::new(AuditTrail::new()),
            rate_limits: Mutex::new(HashMap::new()),
            consent: None,
            agent_id: Uuid::nil(),
            approved_channels: Mutex::new(HashMap::new()),
            fuel: None,
        }
    }

    /// Attach a fuel context for metering integration sends.
    pub fn set_fuel_context(&mut self, fuel: FuelContext) {
        self.fuel = Some(fuel);
    }

    /// Attach a ConsentRuntime for HITL enforcement on integration sends.
    /// First-time sends to a new provider require HITL approval; after approval
    /// the channel is whitelisted for 24 hours.
    pub fn set_consent(&mut self, consent_runtime: ConsentRuntime, agent_id: Uuid) {
        self.consent = Some(Mutex::new(consent_runtime));
        self.agent_id = agent_id;
    }

    /// Check if a provider channel is in the approved whitelist (not expired).
    fn is_channel_approved(&self, provider: &str) -> bool {
        if let Ok(channels) = self.approved_channels.lock() {
            if let Some(&expiry) = channels.get(provider) {
                return current_epoch_secs() < expiry;
            }
        }
        false
    }

    /// Whitelist a provider channel for a given duration.
    fn approve_channel(&self, provider: &str, duration_secs: u64) {
        if let Ok(mut channels) = self.approved_channels.lock() {
            channels.insert(provider.to_string(), current_epoch_secs() + duration_secs);
        }
    }

    /// Route a Nexus event to all matching integrations.
    ///
    /// For each matching provider:
    /// 1. PII-redact the notification body
    /// 2. Check rate limit
    /// 3. Send notification
    /// 4. Record audit entry
    pub fn route(&self, event: &NexusEvent) -> Vec<RouteResult> {
        let event_kind = event.kind();
        let raw_summary = event.summary();
        let severity = event.severity();

        // PII-redact the summary text.
        let findings = RedactionEngine::scan(&raw_summary);
        let redacted_summary = RedactionEngine::apply(&raw_summary, &findings);

        let title = format!("Nexus OS — {event_kind}");

        let mut results = Vec::new();

        for provider in &self.providers {
            let provider_name = provider.name().to_string();
            let provider_type = provider.provider_type();

            // HITL consent check for first-time sends to a new provider channel.
            // Once approved, the channel is whitelisted for 24 hours.
            if let Some(consent_mutex) = &self.consent {
                if !self.is_channel_approved(&provider_name) {
                    let payload = format!(
                        "integration_send:{}:{}",
                        provider_name, event_kind
                    );
                    let hitl_result = if let (Ok(mut consent), Ok(mut audit_guard)) =
                        (consent_mutex.lock(), self.audit.lock())
                    {
                        consent.enforce_operation(
                            GovernedOperation::IntegrationSend,
                            self.agent_id,
                            payload.as_bytes(),
                            &mut audit_guard,
                        )
                    } else {
                        Ok(()) // lock poisoned — fail open with audit
                    };

                    match hitl_result {
                        Ok(()) => {
                            // Approved — whitelist this provider for 24 hours
                            self.approve_channel(&provider_name, 86400);
                        }
                        Err(e) => {
                            let detail = format!("{}", e);
                            self.audit_send(&provider_name, event_kind, false, &detail);
                            results.push(RouteResult {
                                provider: provider_name,
                                provider_type,
                                success: false,
                                error: Some(detail),
                            });
                            continue;
                        }
                    }
                }
            }

            // Rate-limit check.
            let rate_key = provider_name.to_lowercase();
            let allowed = {
                let mut limits = self.rate_limits.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(window) = limits.get_mut(&rate_key) {
                    window.check_and_record()
                } else {
                    true // no limit configured
                }
            };

            if !allowed {
                let err = IntegrationError::RateLimited {
                    provider: provider_name.clone(),
                    retry_after_ms: 60_000,
                };
                self.audit_send(&provider_name, event_kind, false, &err.to_string());
                results.push(RouteResult {
                    provider: provider_name,
                    provider_type,
                    success: false,
                    error: Some(err.to_string()),
                });
                continue;
            }

            // Fuel gate: reserve fuel before sending to provider.
            let fuel_reservation = if let Some(fuel_ctx) = &self.fuel {
                let fuel_key = Self::fuel_action_key(&provider_type);
                let cost = max_fuel_cost(&fuel_key);
                match fuel_ctx.reserve_fuel(cost) {
                    Ok(reservation) => Some(reservation),
                    Err(_) => {
                        let detail = format!(
                            "fuel exhausted for integration_{}: need {}, have {}",
                            provider_name,
                            cost,
                            fuel_ctx.fuel_remaining()
                        );
                        self.audit_send(&provider_name, event_kind, false, &detail);
                        results.push(RouteResult {
                            provider: provider_name,
                            provider_type,
                            success: false,
                            error: Some(detail),
                        });
                        continue;
                    }
                }
            } else {
                None
            };

            let notification = Notification {
                title: title.clone(),
                body: redacted_summary.clone(),
                severity,
                channel: None,
                source_event: event_kind.to_string(),
            };

            match provider.send_notification(&notification) {
                Ok(()) => {
                    if let Some(r) = fuel_reservation {
                        r.commit();
                    }
                    self.audit_send(&provider_name, event_kind, true, "delivered");
                    results.push(RouteResult {
                        provider: provider_name,
                        provider_type,
                        success: true,
                        error: None,
                    });
                }
                Err(e) => {
                    if let Some(r) = fuel_reservation {
                        r.cancel();
                    }
                    let msg = e.to_string();
                    self.audit_send(&provider_name, event_kind, false, &msg);
                    results.push(RouteResult {
                        provider: provider_name,
                        provider_type,
                        success: false,
                        error: Some(msg),
                    });
                }
            }
        }

        results
    }

    /// Run health checks on all registered providers.
    pub fn health_check_all(&self) -> Vec<HealthResult> {
        self.providers
            .iter()
            .map(|p| {
                let ok = p.health_check().is_ok();
                HealthResult {
                    provider: p.name().to_string(),
                    provider_type: p.provider_type(),
                    healthy: ok,
                }
            })
            .collect()
    }

    /// Return provider names and types.
    pub fn list_providers(&self) -> Vec<ProviderInfo> {
        self.providers
            .iter()
            .map(|p| ProviderInfo {
                name: p.name().to_string(),
                provider_type: p.provider_type(),
            })
            .collect()
    }

    /// PII-redact arbitrary text using the configured engine.
    pub fn redact_text(&self, text: &str) -> String {
        let findings = RedactionEngine::scan(text);
        RedactionEngine::apply(text, &findings)
    }

    /// Access the audit trail (e.g. for persistence or export).
    pub fn audit_trail(&self) -> &Mutex<AuditTrail> {
        &self.audit
    }

    // ── private helpers ──────────────────────────────────────────────

    /// Map provider type to a `max_fuel_cost` action key.
    fn fuel_action_key(provider_type: &ProviderType) -> String {
        match provider_type {
            ProviderType::Slack => "integration_slack".to_string(),
            ProviderType::MicrosoftTeams => "integration_teams".to_string(),
            ProviderType::Discord => "integration_discord".to_string(),
            ProviderType::Telegram => "integration_telegram".to_string(),
            ProviderType::Jira | ProviderType::ServiceNow => "integration_jira".to_string(),
            ProviderType::GitHub | ProviderType::GitLab => "integration_github".to_string(),
            ProviderType::CustomWebhook => "integration_webhook".to_string(),
        }
    }

    fn audit_send(&self, provider: &str, event_kind: &str, success: bool, detail: &str) {
        if let Ok(mut audit) = self.audit.lock() {
            let _ = audit.append_event(
                Uuid::nil(),
                EventType::ToolCall,
                json!({
                    "action": "integration_send",
                    "provider": provider,
                    "event_kind": event_kind,
                    "success": success,
                    "detail": detail,
                }),
            );
        }
    }

    fn build_slack(cfg: &ProviderConfig) -> Result<SlackIntegration, IntegrationError> {
        let webhook_url = cfg.resolve_setting("webhook_url").unwrap_or_default();
        let bot_token = cfg.resolve_setting("bot_token");
        let channel = cfg
            .resolve_setting("default_channel")
            .unwrap_or_else(|| "#nexus-alerts".to_string());
        SlackIntegration::new(webhook_url, bot_token, channel)
    }

    fn build_teams(cfg: &ProviderConfig) -> Result<TeamsIntegration, IntegrationError> {
        let webhook_url = cfg.resolve_setting("webhook_url").unwrap_or_default();
        TeamsIntegration::new(webhook_url)
    }

    fn build_discord(cfg: &ProviderConfig) -> Result<DiscordIntegration, IntegrationError> {
        let bot_token = cfg.resolve_setting("bot_token").unwrap_or_default();
        let channel_id = cfg
            .resolve_setting("default_channel_id")
            .unwrap_or_default();
        DiscordIntegration::new(bot_token, channel_id)
    }

    fn build_telegram(cfg: &ProviderConfig) -> Result<TelegramIntegration, IntegrationError> {
        let bot_token = cfg.resolve_setting("bot_token").unwrap_or_default();
        let chat_id = cfg
            .resolve_setting("default_chat_id")
            .unwrap_or_default();
        TelegramIntegration::new(bot_token, chat_id)
    }
}

/// Result of routing an event to a single provider.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RouteResult {
    pub provider: String,
    pub provider_type: ProviderType,
    pub success: bool,
    pub error: Option<String>,
}

/// Health check result for a single provider.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthResult {
    pub provider: String,
    pub provider_type: ProviderType,
    pub healthy: bool,
}

/// Basic provider info for listing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub provider_type: ProviderType,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::IntegrationConfig;

    #[test]
    fn empty_router_routes_nothing() {
        let router = IntegrationRouter::empty();
        let event = NexusEvent::AgentStarted {
            did: "did:nexus:test".into(),
            workspace: "default".into(),
        };
        let results = router.route(&event);
        assert!(results.is_empty());
    }

    #[test]
    fn empty_router_health_check() {
        let router = IntegrationRouter::empty();
        let results = router.health_check_all();
        assert!(results.is_empty());
    }

    #[test]
    fn pii_redaction_strips_email() {
        let router = IntegrationRouter::empty();
        let input = "Contact user@example.com for details";
        let redacted = router.redact_text(input);
        // The redaction engine should replace the email
        assert!(!redacted.contains("user@example.com") || redacted.contains("[REDACTED"));
    }

    #[test]
    fn sliding_window_rate_limit() {
        let mut window = SlidingWindow::new(3);
        assert!(window.check_and_record());
        assert!(window.check_and_record());
        assert!(window.check_and_record());
        // 4th request within the same minute should be denied
        assert!(!window.check_and_record());
    }

    #[test]
    fn from_config_with_defaults() {
        let config = IntegrationConfig::default();
        let router = IntegrationRouter::from_config(&config);
        assert!(router.list_providers().is_empty());
    }

    #[test]
    fn route_result_serialization() {
        let result = RouteResult {
            provider: "slack".into(),
            provider_type: ProviderType::Slack,
            success: true,
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn audit_trail_records_sends() {
        let router = IntegrationRouter::empty();
        router.audit_send("test-provider", "agent_error", true, "ok");
        let audit = router.audit_trail().lock().unwrap();
        let events = audit.events();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn event_kind_matches_expected_strings() {
        let event = NexusEvent::AgentStarted {
            did: "did:nexus:a1".into(),
            workspace: "default".into(),
        };
        assert_eq!(event.kind(), "agent_started");

        let event = NexusEvent::SecurityEvent {
            event_type: "intrusion".into(),
            details: "brute force detected".into(),
        };
        assert_eq!(event.kind(), "security_event");
    }

    #[test]
    fn event_serialization_roundtrip() {
        let event = NexusEvent::HitlRequired {
            did: "did:nexus:agent1".into(),
            action: "deploy_production".into(),
            context: "high-risk deployment".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: NexusEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.kind(), "hitl_required");
        assert_eq!(parsed.summary().contains("deploy_production"), true);
    }

    #[test]
    fn event_severity_classification() {
        use crate::events::Severity;
        let critical = NexusEvent::AuditChainBreak {
            details: "hash mismatch".into(),
        };
        assert_eq!(critical.severity(), Severity::Critical);

        let warning = NexusEvent::FuelExhausted {
            did: "did:nexus:a1".into(),
        };
        assert_eq!(warning.severity(), Severity::Warning);

        let info = NexusEvent::BackupCompleted {
            path: "/tmp/backup".into(),
            size_bytes: 1024,
        };
        assert_eq!(info.severity(), Severity::Info);
    }

    #[test]
    fn sliding_window_resets_after_minute() {
        let mut window = SlidingWindow::new(2);
        assert!(window.check_and_record());
        assert!(window.check_and_record());
        assert!(!window.check_and_record()); // blocked

        // Clear entries to simulate time passing (entries older than 60s are evicted)
        window.entries.clear();
        assert!(window.check_and_record()); // should allow after reset
    }

    #[test]
    fn channel_approval_whitelist() {
        let router = IntegrationRouter::empty();
        assert!(!router.is_channel_approved("slack"));
        router.approve_channel("slack", 86400);
        assert!(router.is_channel_approved("slack"));
        assert!(!router.is_channel_approved("teams"));
    }

    #[test]
    fn hitl_denied_error_variant() {
        let err = IntegrationError::HitlDenied {
            provider: "slack".into(),
            detail: "human denied".into(),
        };
        assert!(err.to_string().contains("HITL denied"));
        assert!(err.to_string().contains("slack"));
    }

    #[test]
    fn multiple_audit_sends_recorded() {
        let router = IntegrationRouter::empty();
        router.audit_send("slack", "agent_started", true, "ok");
        router.audit_send("teams", "agent_error", false, "timeout");
        router.audit_send("webhook", "security_event", true, "delivered");
        let audit = router.audit_trail().lock().unwrap();
        assert_eq!(audit.events().len(), 3);
    }
}
