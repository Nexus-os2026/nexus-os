use nexus_kernel::audit::{AuditEvent, AuditTrail, EventType};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionType {
    ScreenCapture,
    WindowCapture,
    MouseMove,
    MouseClick,
    TypeText,
    KeyPress,
    BrowserNavigate,
    BrowserClick,
    BrowserType,
    BrowserContentRead,
    BrowserScreenshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionMetadata {
    pub timestamp: u64,
    pub action_type: ActionType,
    pub coordinates: Option<(i32, i32)>,
    pub target_window: Option<String>,
    pub agent_id: Uuid,
}

#[derive(Clone, Default)]
pub struct ActionLogger {
    audit_trail: Arc<Mutex<AuditTrail>>,
}

impl ActionLogger {
    pub fn new() -> Self {
        Self {
            audit_trail: Arc::new(Mutex::new(AuditTrail::new())),
        }
    }

    pub fn log_action(
        &self,
        action_type: ActionType,
        agent_id: Uuid,
        coordinates: Option<(i32, i32)>,
        target_window: Option<&str>,
        details: Value,
    ) -> Uuid {
        let timestamp = current_unix_timestamp();
        let metadata = ActionMetadata {
            timestamp,
            action_type,
            coordinates,
            target_window: target_window.map(ToString::to_string),
            agent_id,
        };

        let payload = json!({
            "timestamp": metadata.timestamp,
            "action_type": format!("{:?}", metadata.action_type),
            "coordinates": metadata.coordinates,
            "target_window": metadata.target_window,
            "agent_id": metadata.agent_id,
            "details": details,
        });

        let mut guard = match self.audit_trail.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        guard.append_event(agent_id, EventType::ToolCall, payload)
    }

    pub fn events(&self) -> Vec<AuditEvent> {
        let guard = match self.audit_trail.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.events().to_vec()
    }

    pub fn verify_integrity(&self) -> bool {
        let guard = match self.audit_trail.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.verify_integrity()
    }
}

impl std::fmt::Debug for ActionLogger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let event_count = self.events().len();
        f.debug_struct("ActionLogger")
            .field("event_count", &event_count)
            .finish()
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
    use super::{ActionLogger, ActionType};
    use crate::capture::{MockCaptureBackend, ScreenCaptureService};
    use crate::input::{InputController, MockInputBackend, MouseButton};
    use crate::ControlAgentContext;
    use serde_json::json;
    use std::collections::HashSet;
    use uuid::Uuid;

    fn context_with_caps(caps: &[&str]) -> ControlAgentContext {
        let capabilities = caps.iter().map(|cap| (*cap).to_string()).collect::<HashSet<_>>();
        ControlAgentContext::new(Uuid::new_v4(), capabilities)
    }

    #[test]
    fn test_action_logger_integrity() {
        let logger = ActionLogger::new();
        let agent_id = Uuid::new_v4();

        let _ = logger.log_action(
            ActionType::MouseClick,
            agent_id,
            Some((10, 20)),
            None,
            json!({"button": "left"}),
        );
        let _ = logger.log_action(
            ActionType::TypeText,
            agent_id,
            None,
            None,
            json!({"chars": 4}),
        );

        assert_eq!(logger.events().len(), 2);
        assert!(logger.verify_integrity());
    }

    #[test]
    fn test_action_logging() {
        let logger = ActionLogger::new();
        let context = context_with_caps(&["screen.capture", "input.mouse"]);

        let mut capture = ScreenCaptureService::new(MockCaptureBackend::default(), logger.clone());
        let mut input = InputController::new(MockInputBackend::default(), logger.clone());

        let capture_result = capture.capture_screen(&context);
        assert!(capture_result.is_ok());

        let click_result = input.click(&context, 100, 200, MouseButton::Left);
        assert!(click_result.is_ok());

        let events = logger.events();
        assert_eq!(events.len(), 2);

        let action_types = events
            .iter()
            .filter_map(|event| event.payload.get("action_type").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();
        assert!(action_types.contains(&"ScreenCapture"));
        assert!(action_types.contains(&"MouseClick"));
    }
}
