use crate::action_log::{ActionLogger, ActionType};
use crate::ControlAgentContext;
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

pub trait InputBackend {
    fn move_mouse(&mut self, x: i32, y: i32) -> Result<(), AgentError>;
    fn click(&mut self, x: i32, y: i32, button: MouseButton) -> Result<(), AgentError>;
    fn type_text(&mut self, text: &str) -> Result<(), AgentError>;
    fn key_press(&mut self, key: &str) -> Result<(), AgentError>;
}

#[derive(Debug, Clone, Default)]
pub struct MockInputBackend {
    actions: Vec<String>,
}

impl MockInputBackend {
    pub fn actions(&self) -> &[String] {
        &self.actions
    }
}

impl InputBackend for MockInputBackend {
    fn move_mouse(&mut self, x: i32, y: i32) -> Result<(), AgentError> {
        self.actions.push(format!("move:{x},{y}"));
        Ok(())
    }

    fn click(&mut self, x: i32, y: i32, button: MouseButton) -> Result<(), AgentError> {
        self.actions.push(format!("click:{button:?}@{x},{y}"));
        Ok(())
    }

    fn type_text(&mut self, text: &str) -> Result<(), AgentError> {
        self.actions.push(format!("type:{}", text.len()));
        Ok(())
    }

    fn key_press(&mut self, key: &str) -> Result<(), AgentError> {
        self.actions.push(format!("key:{key}"));
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct InputController<B: InputBackend> {
    backend: B,
    action_logger: ActionLogger,
}

impl<B: InputBackend> InputController<B> {
    pub fn new(backend: B, action_logger: ActionLogger) -> Self {
        Self {
            backend,
            action_logger,
        }
    }

    pub fn move_mouse(
        &mut self,
        context: &ControlAgentContext,
        x: i32,
        y: i32,
    ) -> Result<(), AgentError> {
        ensure_capability(context, "input.mouse")?;
        self.backend.move_mouse(x, y)?;

        let _ = self.action_logger.log_action(
            ActionType::MouseMove,
            context.agent_id,
            Some((x, y)),
            None,
            json!({}),
        );

        Ok(())
    }

    pub fn click(
        &mut self,
        context: &ControlAgentContext,
        x: i32,
        y: i32,
        button: MouseButton,
    ) -> Result<(), AgentError> {
        ensure_capability(context, "input.mouse")?;
        self.backend.click(x, y, button)?;

        let _ = self.action_logger.log_action(
            ActionType::MouseClick,
            context.agent_id,
            Some((x, y)),
            None,
            json!({"button": format!("{button:?}")}),
        );

        Ok(())
    }

    pub fn type_text(
        &mut self,
        context: &ControlAgentContext,
        text: &str,
    ) -> Result<(), AgentError> {
        ensure_capability(context, "input.keyboard")?;
        self.backend.type_text(text)?;

        let _ = self.action_logger.log_action(
            ActionType::TypeText,
            context.agent_id,
            None,
            None,
            json!({"text_length": text.chars().count()}),
        );

        Ok(())
    }

    pub fn key_press(
        &mut self,
        context: &ControlAgentContext,
        key: &str,
    ) -> Result<(), AgentError> {
        ensure_capability(context, "input.keyboard")?;
        self.backend.key_press(key)?;

        let _ = self.action_logger.log_action(
            ActionType::KeyPress,
            context.agent_id,
            None,
            None,
            json!({"key": key}),
        );

        Ok(())
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
pub struct LinuxPlatformInputBackend;

#[cfg(all(target_os = "linux", feature = "platform-linux"))]
impl InputBackend for LinuxPlatformInputBackend {
    fn move_mouse(&mut self, _x: i32, _y: i32) -> Result<(), AgentError> {
        Err(AgentError::SupervisorError(
            "linux input backend not wired in this build".to_string(),
        ))
    }

    fn click(&mut self, _x: i32, _y: i32, _button: MouseButton) -> Result<(), AgentError> {
        Err(AgentError::SupervisorError(
            "linux click backend not wired in this build".to_string(),
        ))
    }

    fn type_text(&mut self, _text: &str) -> Result<(), AgentError> {
        Err(AgentError::SupervisorError(
            "linux keyboard backend not wired in this build".to_string(),
        ))
    }

    fn key_press(&mut self, _key: &str) -> Result<(), AgentError> {
        Err(AgentError::SupervisorError(
            "linux key backend not wired in this build".to_string(),
        ))
    }
}

#[cfg(all(target_os = "macos", feature = "platform-macos"))]
pub struct MacOsPlatformInputBackend;

#[cfg(all(target_os = "macos", feature = "platform-macos"))]
impl InputBackend for MacOsPlatformInputBackend {
    fn move_mouse(&mut self, _x: i32, _y: i32) -> Result<(), AgentError> {
        Err(AgentError::SupervisorError(
            "macOS input backend not wired in this build".to_string(),
        ))
    }

    fn click(&mut self, _x: i32, _y: i32, _button: MouseButton) -> Result<(), AgentError> {
        Err(AgentError::SupervisorError(
            "macOS click backend not wired in this build".to_string(),
        ))
    }

    fn type_text(&mut self, _text: &str) -> Result<(), AgentError> {
        Err(AgentError::SupervisorError(
            "macOS keyboard backend not wired in this build".to_string(),
        ))
    }

    fn key_press(&mut self, _key: &str) -> Result<(), AgentError> {
        Err(AgentError::SupervisorError(
            "macOS key backend not wired in this build".to_string(),
        ))
    }
}

#[cfg(all(target_os = "windows", feature = "platform-windows"))]
pub struct WindowsPlatformInputBackend;

#[cfg(all(target_os = "windows", feature = "platform-windows"))]
impl InputBackend for WindowsPlatformInputBackend {
    fn move_mouse(&mut self, _x: i32, _y: i32) -> Result<(), AgentError> {
        Err(AgentError::SupervisorError(
            "windows input backend not wired in this build".to_string(),
        ))
    }

    fn click(&mut self, _x: i32, _y: i32, _button: MouseButton) -> Result<(), AgentError> {
        Err(AgentError::SupervisorError(
            "windows click backend not wired in this build".to_string(),
        ))
    }

    fn type_text(&mut self, _text: &str) -> Result<(), AgentError> {
        Err(AgentError::SupervisorError(
            "windows keyboard backend not wired in this build".to_string(),
        ))
    }

    fn key_press(&mut self, _key: &str) -> Result<(), AgentError> {
        Err(AgentError::SupervisorError(
            "windows key backend not wired in this build".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{InputController, MockInputBackend};
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
    fn test_input_governed() {
        let logger = ActionLogger::new();
        let mut input = InputController::new(MockInputBackend::default(), logger);
        let context = context_with_caps(&["screen.capture"]);

        let result = input.click(&context, 10, 10, super::MouseButton::Left);
        assert_eq!(
            result,
            Err(AgentError::CapabilityDenied("input.mouse".to_string()))
        );
    }
}
