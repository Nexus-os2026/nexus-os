use crate::action_log::{ActionLogger, ActionType};
use crate::browser::policy::DomainPolicy;
use crate::capture::{MockCaptureBackend, ScreenCaptureService, Screenshot};
use crate::ControlAgentContext;
use nexus_connectors_core::rate_limit::{RateLimitDecision, RateLimiter};
use nexus_kernel::errors::AgentError;
use serde_json::json;

pub trait BrowserRuntime {
    fn navigate(&mut self, url: &str) -> Result<(), AgentError>;
    fn click_element(&mut self, selector: &str) -> Result<(), AgentError>;
    fn type_in_element(&mut self, selector: &str, text: &str) -> Result<(), AgentError>;
    fn get_page_content(&self) -> Result<String, AgentError>;
}

#[derive(Debug, Clone, Default)]
pub struct MockBrowserRuntime {
    current_url: Option<String>,
    content: String,
}

impl MockBrowserRuntime {
    pub fn set_content(&mut self, content: impl Into<String>) {
        self.content = content.into();
    }
}

impl BrowserRuntime for MockBrowserRuntime {
    fn navigate(&mut self, url: &str) -> Result<(), AgentError> {
        self.current_url = Some(url.to_string());
        self.content = format!(
            "page content for {}. navigation links and scripts removed",
            url
        );
        Ok(())
    }

    fn click_element(&mut self, _selector: &str) -> Result<(), AgentError> {
        Ok(())
    }

    fn type_in_element(&mut self, _selector: &str, _text: &str) -> Result<(), AgentError> {
        Ok(())
    }

    fn get_page_content(&self) -> Result<String, AgentError> {
        Ok(self.content.clone())
    }
}

#[cfg(feature = "playwright-process")]
#[derive(Debug, Clone)]
pub struct PlaywrightProcessRuntime {
    executable: String,
}

#[cfg(feature = "playwright-process")]
impl PlaywrightProcessRuntime {
    pub fn new(executable: impl Into<String>) -> Self {
        Self {
            executable: executable.into(),
        }
    }
}

#[cfg(feature = "playwright-process")]
impl BrowserRuntime for PlaywrightProcessRuntime {
    fn navigate(&mut self, _url: &str) -> Result<(), AgentError> {
        let _ = self.executable.as_str();
        Err(AgentError::SupervisorError(
            "playwright runtime not wired in this build".to_string(),
        ))
    }

    fn click_element(&mut self, _selector: &str) -> Result<(), AgentError> {
        Err(AgentError::SupervisorError(
            "playwright runtime not wired in this build".to_string(),
        ))
    }

    fn type_in_element(&mut self, _selector: &str, _text: &str) -> Result<(), AgentError> {
        Err(AgentError::SupervisorError(
            "playwright runtime not wired in this build".to_string(),
        ))
    }

    fn get_page_content(&self) -> Result<String, AgentError> {
        Err(AgentError::SupervisorError(
            "playwright runtime not wired in this build".to_string(),
        ))
    }
}

#[derive(Clone)]
pub struct GovernedBrowser<R: BrowserRuntime> {
    runtime: R,
    policy: DomainPolicy,
    logger: ActionLogger,
    capture_service: ScreenCaptureService<MockCaptureBackend>,
    rate_limiter: RateLimiter,
}

impl<R: BrowserRuntime> GovernedBrowser<R> {
    pub fn new(runtime: R, policy: DomainPolicy, logger: ActionLogger) -> Self {
        let limiter = RateLimiter::new();
        limiter.configure("browser", 120, 60);

        Self {
            runtime,
            policy,
            capture_service: ScreenCaptureService::new(
                MockCaptureBackend::default(),
                logger.clone(),
            ),
            logger,
            rate_limiter: limiter,
        }
    }

    pub fn navigate(&mut self, context: &ControlAgentContext, url: &str) -> Result<(), AgentError> {
        ensure_capability(context, "browser.navigate")?;
        self.ensure_not_rate_limited()?;

        if !self.policy.is_allowed_url(url) {
            let _ = self.logger.log_action(
                ActionType::BrowserNavigate,
                context.agent_id,
                None,
                None,
                json!({"url": url, "status": "blocked"}),
            );
            return Err(AgentError::CapabilityDenied(format!(
                "domain '{}' is not in allowlist",
                url
            )));
        }

        self.runtime.navigate(url)?;
        let _ = self.logger.log_action(
            ActionType::BrowserNavigate,
            context.agent_id,
            None,
            None,
            json!({"url": url, "status": "ok"}),
        );
        Ok(())
    }

    pub fn click_element(
        &mut self,
        context: &ControlAgentContext,
        selector: &str,
    ) -> Result<(), AgentError> {
        ensure_capability(context, "input.mouse")?;
        self.ensure_not_rate_limited()?;

        self.runtime.click_element(selector)?;
        let _ = self.logger.log_action(
            ActionType::BrowserClick,
            context.agent_id,
            None,
            None,
            json!({"selector": selector}),
        );

        Ok(())
    }

    pub fn type_in_element(
        &mut self,
        context: &ControlAgentContext,
        selector: &str,
        text: &str,
    ) -> Result<(), AgentError> {
        ensure_capability(context, "input.keyboard")?;
        self.ensure_not_rate_limited()?;

        self.runtime.type_in_element(selector, text)?;
        let _ = self.logger.log_action(
            ActionType::BrowserType,
            context.agent_id,
            None,
            None,
            json!({"selector": selector, "text_length": text.chars().count()}),
        );

        Ok(())
    }

    pub fn get_page_content(&self, context: &ControlAgentContext) -> Result<String, AgentError> {
        ensure_capability(context, "web.read")?;
        let content = self.runtime.get_page_content()?;
        let clean = clean_page_text(content.as_str());
        let _ = self.logger.log_action(
            ActionType::BrowserContentRead,
            context.agent_id,
            None,
            None,
            json!({"text_length": clean.chars().count()}),
        );
        Ok(clean)
    }

    pub fn screenshot(&mut self, context: &ControlAgentContext) -> Result<Screenshot, AgentError> {
        ensure_capability(context, "screen.capture")?;
        let shot = self.capture_service.capture_screen(context)?;
        let _ = self.logger.log_action(
            ActionType::BrowserScreenshot,
            context.agent_id,
            None,
            None,
            json!({"width": shot.width, "height": shot.height}),
        );
        Ok(shot)
    }

    pub fn action_logger(&self) -> &ActionLogger {
        &self.logger
    }

    fn ensure_not_rate_limited(&self) -> Result<(), AgentError> {
        match self.rate_limiter.check("browser") {
            RateLimitDecision::Allowed => Ok(()),
            RateLimitDecision::RateLimited { retry_after_ms } => Err(AgentError::SupervisorError(
                format!("browser rate limited, retry after {retry_after_ms} ms"),
            )),
        }
    }
}

fn ensure_capability(context: &ControlAgentContext, capability: &str) -> Result<(), AgentError> {
    if !context.has_capability(capability) {
        return Err(AgentError::CapabilityDenied(capability.to_string()));
    }

    Ok(())
}

fn clean_page_text(raw: &str) -> String {
    let mut in_tag = false;
    let mut plain = String::new();

    for ch in raw.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => plain.push(ch),
            _ => {}
        }
    }

    plain.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::{GovernedBrowser, MockBrowserRuntime};
    use crate::action_log::ActionLogger;
    use crate::browser::policy::DomainPolicy;
    use crate::ControlAgentContext;
    use nexus_kernel::errors::AgentError;
    use std::collections::HashSet;
    use uuid::Uuid;

    fn context_with_caps(caps: &[&str]) -> ControlAgentContext {
        let capabilities = caps
            .iter()
            .map(|cap| (*cap).to_string())
            .collect::<HashSet<_>>();
        ControlAgentContext::new(Uuid::new_v4(), capabilities)
    }

    #[test]
    fn test_navigate_allowed_domain() {
        let runtime = MockBrowserRuntime::default();
        let policy = DomainPolicy::new(vec!["*.github.com".to_string()]);
        let logger = ActionLogger::new();
        let mut browser = GovernedBrowser::new(runtime, policy, logger);

        let context = context_with_caps(&["browser.navigate"]);
        let result = browser.navigate(&context, "https://github.com/nex-lang");
        assert!(result.is_ok());
    }

    #[test]
    fn test_navigate_blocked_domain() {
        let runtime = MockBrowserRuntime::default();
        let policy = DomainPolicy::new(vec!["*.github.com".to_string()]);
        let logger = ActionLogger::new();
        let mut browser = GovernedBrowser::new(runtime, policy, logger);

        let context = context_with_caps(&["browser.navigate"]);
        let result = browser.navigate(&context, "https://evil.com");
        assert!(matches!(result, Err(AgentError::CapabilityDenied(_))));
    }

    #[test]
    fn test_all_actions_logged() {
        let runtime = MockBrowserRuntime::default();
        let policy = DomainPolicy::new(vec!["*.github.com".to_string()]);
        let logger = ActionLogger::new();
        let mut browser = GovernedBrowser::new(runtime, policy, logger.clone());

        let context = context_with_caps(&["browser.navigate", "input.mouse", "input.keyboard"]);

        assert!(browser
            .navigate(&context, "https://github.com/nex-lang")
            .is_ok());
        assert!(browser.click_element(&context, "#submit").is_ok());
        assert!(browser
            .type_in_element(&context, "#prompt", "hello")
            .is_ok());

        let events = logger.events();
        assert_eq!(events.len(), 3);

        let action_types = events
            .iter()
            .filter_map(|event| {
                event
                    .payload
                    .get("action_type")
                    .and_then(|value| value.as_str())
            })
            .collect::<Vec<_>>();
        assert!(action_types.contains(&"BrowserNavigate"));
        assert!(action_types.contains(&"BrowserClick"));
        assert!(action_types.contains(&"BrowserType"));
    }
}
