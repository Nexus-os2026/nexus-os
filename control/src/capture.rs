use crate::action_log::{ActionLogger, ActionType};
use crate::ControlAgentContext;
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screenshot {
    pub width: u32,
    pub height: u32,
    pub pixels_rgba: Vec<u8>,
    pub captured_at: u64,
    pub window_id: Option<String>,
    pub platform: String,
}

pub trait ScreenCaptureBackend {
    fn capture_screen(&mut self) -> Result<Screenshot, AgentError>;
    fn capture_window(&mut self, window_id: &str) -> Result<Screenshot, AgentError>;
}

#[derive(Debug, Clone)]
pub struct MockCaptureBackend {
    width: u32,
    height: u32,
}

impl MockCaptureBackend {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl Default for MockCaptureBackend {
    fn default() -> Self {
        Self::new(64, 64)
    }
}

impl ScreenCaptureBackend for MockCaptureBackend {
    fn capture_screen(&mut self) -> Result<Screenshot, AgentError> {
        Ok(Screenshot {
            width: self.width,
            height: self.height,
            pixels_rgba: vec![42_u8; (self.width as usize) * (self.height as usize) * 4],
            captured_at: current_unix_timestamp(),
            window_id: None,
            platform: current_platform_name(),
        })
    }

    fn capture_window(&mut self, window_id: &str) -> Result<Screenshot, AgentError> {
        Ok(Screenshot {
            width: self.width,
            height: self.height,
            pixels_rgba: vec![24_u8; (self.width as usize) * (self.height as usize) * 4],
            captured_at: current_unix_timestamp(),
            window_id: Some(window_id.to_string()),
            platform: current_platform_name(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct ScreenCaptureService<B: ScreenCaptureBackend> {
    backend: B,
    action_logger: ActionLogger,
}

impl<B: ScreenCaptureBackend> ScreenCaptureService<B> {
    pub fn new(backend: B, action_logger: ActionLogger) -> Self {
        Self {
            backend,
            action_logger,
        }
    }

    pub fn capture_screen(
        &mut self,
        context: &ControlAgentContext,
    ) -> Result<Screenshot, AgentError> {
        ensure_capability(context, "screen.capture")?;

        let screenshot = self.backend.capture_screen()?;
        let _ = self.action_logger.log_action(
            ActionType::ScreenCapture,
            context.agent_id,
            None,
            None,
            json!({
                "width": screenshot.width,
                "height": screenshot.height,
                "platform": screenshot.platform
            }),
        );

        Ok(screenshot)
    }

    pub fn capture_window(
        &mut self,
        context: &ControlAgentContext,
        window_id: &str,
    ) -> Result<Screenshot, AgentError> {
        ensure_capability(context, "screen.capture")?;

        let screenshot = self.backend.capture_window(window_id)?;
        let _ = self.action_logger.log_action(
            ActionType::WindowCapture,
            context.agent_id,
            None,
            Some(window_id),
            json!({
                "width": screenshot.width,
                "height": screenshot.height,
                "platform": screenshot.platform
            }),
        );

        Ok(screenshot)
    }

    pub fn action_logger(&self) -> &ActionLogger {
        &self.action_logger
    }
}

fn ensure_capability(context: &ControlAgentContext, capability: &str) -> Result<(), AgentError> {
    if !context.has_capability(capability) {
        return Err(AgentError::CapabilityDenied(capability.to_string()));
    }

    Ok(())
}

#[cfg(all(target_os = "linux", feature = "platform-linux"))]
pub struct LinuxPlatformCaptureBackend;

#[cfg(all(target_os = "linux", feature = "platform-linux"))]
impl ScreenCaptureBackend for LinuxPlatformCaptureBackend {
    fn capture_screen(&mut self) -> Result<Screenshot, AgentError> {
        Err(AgentError::SupervisorError(
            "linux capture backend not wired in this build".to_string(),
        ))
    }

    fn capture_window(&mut self, _window_id: &str) -> Result<Screenshot, AgentError> {
        Err(AgentError::SupervisorError(
            "linux window capture backend not wired in this build".to_string(),
        ))
    }
}

#[cfg(all(target_os = "macos", feature = "platform-macos"))]
pub struct MacOsPlatformCaptureBackend;

#[cfg(all(target_os = "macos", feature = "platform-macos"))]
impl ScreenCaptureBackend for MacOsPlatformCaptureBackend {
    fn capture_screen(&mut self) -> Result<Screenshot, AgentError> {
        Err(AgentError::SupervisorError(
            "macOS capture backend not wired in this build".to_string(),
        ))
    }

    fn capture_window(&mut self, _window_id: &str) -> Result<Screenshot, AgentError> {
        Err(AgentError::SupervisorError(
            "macOS window capture backend not wired in this build".to_string(),
        ))
    }
}

#[cfg(all(target_os = "windows", feature = "platform-windows"))]
pub struct WindowsPlatformCaptureBackend;

#[cfg(all(target_os = "windows", feature = "platform-windows"))]
impl ScreenCaptureBackend for WindowsPlatformCaptureBackend {
    fn capture_screen(&mut self) -> Result<Screenshot, AgentError> {
        Err(AgentError::SupervisorError(
            "windows capture backend not wired in this build".to_string(),
        ))
    }

    fn capture_window(&mut self, _window_id: &str) -> Result<Screenshot, AgentError> {
        Err(AgentError::SupervisorError(
            "windows window capture backend not wired in this build".to_string(),
        ))
    }
}

fn current_platform_name() -> String {
    if cfg!(target_os = "linux") {
        "linux".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else {
        "unknown".to_string()
    }
}

fn current_unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{MockCaptureBackend, ScreenCaptureService};
    use crate::action_log::ActionLogger;
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
    fn test_capture_governed() {
        let logger = ActionLogger::new();
        let mut capture = ScreenCaptureService::new(MockCaptureBackend::default(), logger);
        let context = context_with_caps(&["input.mouse"]);

        let result = capture.capture_screen(&context);
        assert_eq!(
            result,
            Err(AgentError::CapabilityDenied("screen.capture".to_string()))
        );
    }
}
