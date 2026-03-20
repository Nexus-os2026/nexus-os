use crate::approval::ApprovedDraft;
use crate::navigator::{NavigationResult, PlatformNavigator, SocialPlatform};
use crate::stealth::{gaussian_action_delays_ms, typing_delays_ms, SessionGuard, StealthProfile};
use nexus_sdk::audit::{AuditEvent, AuditTrail, EventType};
use nexus_sdk::errors::AgentError;
use nexus_sdk::kill_gates::{GateStatus, KillGateRegistry};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub trait NavigatorBrowser {
    fn open(&mut self, url: &str) -> Result<(), AgentError>;
    fn has_selector(&mut self, selector: &str) -> Result<bool, AgentError>;
}

pub trait NavigatorVision {
    fn check_logged_in(&mut self, platform: SocialPlatform) -> Result<bool, AgentError>;
    fn find_ui_element(
        &mut self,
        platform: SocialPlatform,
        element_name: &str,
    ) -> Result<Option<String>, AgentError>;
}

pub trait PostingRuntime {
    fn click(&mut self, locator: &str) -> Result<(), AgentError>;
    fn type_char(&mut self, ch: char) -> Result<(), AgentError>;
    fn upload_media(&mut self, media_path: &str) -> Result<(), AgentError>;
    fn screenshot(&mut self, label: &str) -> Result<String, AgentError>;
}

pub trait PostVision {
    fn verify_page(&mut self, platform: SocialPlatform) -> Result<bool, AgentError>;
    fn verify_preview(&mut self, content: &str) -> Result<bool, AgentError>;
    fn verify_posted(&mut self) -> Result<bool, AgentError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostResult {
    pub posted: bool,
    pub verification_passed: bool,
    pub typing_delays_ms: Vec<u64>,
    pub inter_action_delays_ms: Vec<u64>,
    pub screenshots: Vec<String>,
    pub platform: SocialPlatform,
}

pub struct ScreenPosterEngine<B, V, R, P>
where
    B: crate::navigator::NavigatorBrowser,
    V: crate::navigator::NavigatorVision,
    R: PostingRuntime,
    P: PostVision,
{
    navigator: PlatformNavigator<B, V>,
    runtime: R,
    vision: P,
    stealth: StealthProfile,
    session_guard: SessionGuard,
    audit_trail: AuditTrail,
    agent_id: Uuid,
    sleep_enabled: bool,
    kill_gates: KillGateRegistry,
    ban_rate_percent: f64,
}

impl<B, V, R, P> ScreenPosterEngine<B, V, R, P>
where
    B: crate::navigator::NavigatorBrowser,
    V: crate::navigator::NavigatorVision,
    R: PostingRuntime,
    P: PostVision,
{
    pub fn new(navigator: PlatformNavigator<B, V>, runtime: R, vision: P) -> Self {
        let stealth = StealthProfile::default();
        let session_guard = SessionGuard::new(stealth.clone());
        Self {
            navigator,
            runtime,
            vision,
            stealth,
            session_guard,
            audit_trail: AuditTrail::new(),
            agent_id: Uuid::new_v4(),
            sleep_enabled: false,
            kill_gates: KillGateRegistry::default(),
            ban_rate_percent: 0.0,
        }
    }

    pub fn with_sleep(mut self, enabled: bool) -> Self {
        self.sleep_enabled = enabled;
        self
    }

    pub fn with_ban_rate_percent(mut self, value: f64) -> Self {
        self.ban_rate_percent = value.max(0.0);
        self
    }

    pub fn post(
        &mut self,
        approved_draft: &ApprovedDraft,
        platform: SocialPlatform,
    ) -> Result<PostResult, AgentError> {
        match self.kill_gates.check_gate(
            "screen_poster",
            self.ban_rate_percent,
            self.agent_id,
            &mut self.audit_trail,
        ) {
            GateStatus::Open => {}
            GateStatus::Frozen => {
                return Err(AgentError::SupervisorError(
                    "screen_poster kill gate frozen due to ban-rate threshold".to_string(),
                ))
            }
            GateStatus::Halted => {
                return Err(AgentError::SupervisorError(
                    "screen_poster kill gate halted due to ban-rate threshold".to_string(),
                ))
            }
        }

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);

        if !self.session_guard.allow_session(0) {
            return Err(AgentError::SupervisorError(
                "session limit reached".to_string(),
            ));
        }
        if !self.session_guard.allow_post(now_secs) {
            return Err(AgentError::SupervisorError(
                "posting rate limit reached for current hour".to_string(),
            ));
        }

        let navigation = self.navigator.navigate_to_platform(platform)?;
        self.audit_navigation(&navigation);
        if !navigation.logged_in {
            return Err(AgentError::SupervisorError(
                "platform session is not logged in".to_string(),
            ));
        }
        if !self.vision.verify_page(platform)? {
            return Err(AgentError::SupervisorError(
                "vision verification failed for target page".to_string(),
            ));
        }

        let mut screenshots = Vec::new();
        screenshots.push(self.runtime.screenshot("step-1-navigation")?);

        self.runtime.click(navigation.new_post_locator.as_str())?;
        self.audit_step("open_new_post", platform, true);

        let typing_delays_ms = self.type_human_like(approved_draft.draft.text.as_str())?;

        if !approved_draft.draft.media_urls.is_empty() {
            for media in &approved_draft.draft.media_urls {
                self.runtime.upload_media(media.as_str())?;
            }
            self.audit_step("upload_media", platform, true);
        }

        if !approved_draft.draft.hashtags.is_empty() {
            self.runtime.type_char('\n')?;
            let tags = approved_draft.draft.hashtags.join(" ");
            for ch in tags.chars() {
                self.runtime.type_char(ch)?;
            }
            self.audit_step("append_hashtags", platform, true);
        }

        screenshots.push(self.runtime.screenshot("step-2-preview")?);
        let preview_ok = self
            .vision
            .verify_preview(approved_draft.draft.text.as_str())?;
        if !preview_ok {
            return Ok(PostResult {
                posted: false,
                verification_passed: false,
                typing_delays_ms,
                inter_action_delays_ms: Vec::new(),
                screenshots,
                platform,
            });
        }

        let inter_action_delays_ms = gaussian_action_delays_ms(10, &self.stealth);
        for delay_ms in &inter_action_delays_ms {
            maybe_sleep(*delay_ms, self.sleep_enabled);
        }

        self.runtime.click(navigation.publish_locator.as_str())?;
        self.audit_step("publish", platform, true);
        screenshots.push(self.runtime.screenshot("step-3-after-publish")?);

        let posted = self.vision.verify_posted()?;
        if let Err(e) = self.audit_trail.append_event(
            self.agent_id,
            EventType::ToolCall,
            json!({
                "step": "post_verification",
                "platform": platform.as_label(),
                "posted": posted,
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }

        Ok(PostResult {
            posted,
            verification_passed: posted,
            typing_delays_ms,
            inter_action_delays_ms,
            screenshots,
            platform,
        })
    }

    pub fn simulate_human_typing(&self, text: &str) -> Vec<u64> {
        typing_delays_ms(text, &self.stealth)
    }

    pub fn audit_events(&self) -> &[AuditEvent] {
        self.audit_trail.events()
    }

    fn type_human_like(&mut self, text: &str) -> Result<Vec<u64>, AgentError> {
        let delays = self.simulate_human_typing(text);
        for (ch, delay_ms) in text.chars().zip(delays.iter()) {
            self.runtime.type_char(ch)?;
            maybe_sleep(*delay_ms, self.sleep_enabled);
        }
        self.audit_step("type_content", SocialPlatform::X, true);
        Ok(delays)
    }

    fn audit_navigation(&mut self, navigation: &NavigationResult) {
        if let Err(e) = self.audit_trail.append_event(
            self.agent_id,
            EventType::ToolCall,
            json!({
                "step": "navigate",
                "platform": navigation.platform.as_label(),
                "url": navigation.url,
                "logged_in": navigation.logged_in,
                "used_vision_fallback": navigation.used_vision_fallback,
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }
    }

    fn audit_step(&mut self, step: &str, platform: SocialPlatform, ok: bool) {
        if let Err(e) = self.audit_trail.append_event(
            self.agent_id,
            EventType::ToolCall,
            json!({
                "step": step,
                "platform": platform.as_label(),
                "ok": ok,
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }
    }
}

fn maybe_sleep(delay_ms: u64, enabled: bool) {
    if !enabled {
        return;
    }
    thread::sleep(Duration::from_millis(delay_ms));
}
